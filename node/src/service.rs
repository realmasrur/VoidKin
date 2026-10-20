//! VOIDKIN node service: Aura (authoring) + GRANDPA (finality) + Frontier (Ethereum).
//! Structure follows frontier/template/node/src/service.rs. When pinning a release,
//! diff against that file: sc-service / fc-* signatures shift between releases.
use std::{collections::BTreeMap, path::PathBuf, sync::{Arc, Mutex}, time::Duration};

use futures::{channel::mpsc, prelude::*};
use sc_client_api::{Backend, BlockBackend};
use sc_consensus_grandpa::SharedVoterState;
use sc_service::{error::Error as ServiceError, Configuration, TaskManager, WarpSyncConfig};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_consensus_aura::sr25519::AuthorityPair as AuraPair;
use voidkin_runtime::opaque::Block;

use crate::{cli::EthConfiguration, rpc};

pub type HostFunctions = sp_io::SubstrateHostFunctions;
pub type FullClient = sc_service::TFullClient<Block, voidkin_runtime::RuntimeApi, sc_executor::WasmExecutor<HostFunctions>>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type GrandpaBlockImport = sc_consensus_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;

pub fn db_config_dir(config: &Configuration) -> PathBuf {
    config.base_path.config_dir(config.chain_spec.id())
}

pub fn frontier_database_dir(config: &Configuration, path: &str) -> PathBuf {
    db_config_dir(config).join("frontier").join(path)
}

pub fn open_frontier_backend<C: sp_blockchain::HeaderBackend<Block>>(
    client: Arc<C>, config: &Configuration,
) -> Result<Arc<fc_db::kv::Backend<Block, C>>, String> {
    Ok(Arc::new(fc_db::kv::Backend::<Block, C>::new(
        client,
        &fc_db::kv::DatabaseSettings {
            source: sc_client_db::DatabaseSource::RocksDb {
                path: frontier_database_dir(config, "db"),
                cache_size: 0,
            },
        },
    )?))
}

pub fn new_partial(config: &Configuration, eth_config: &EthConfiguration) -> Result<
    sc_service::PartialComponents<
        FullClient, FullBackend, FullSelectChain,
        sc_consensus::DefaultImportQueue<Block>,
        sc_transaction_pool::FullPool<Block, FullClient>,
        (
            Option<Telemetry>,
            GrandpaBlockImport,
            sc_consensus_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
            Arc<fc_db::kv::Backend<Block, FullClient>>,
        ),
    >, ServiceError>
{
    let telemetry = config.telemetry_endpoints.clone().filter(|x| !x.is_empty()).map(|endpoints| -> Result<_, sc_telemetry::Error> {
        let worker = TelemetryWorker::new(16)?;
        let telemetry = worker.handle().new_telemetry(endpoints);
        Ok((worker, telemetry))
    }).transpose()?;

    let executor = sc_service::new_wasm_executor::<HostFunctions>(config);
    let (client, backend, keystore_container, task_manager) = sc_service::new_full_parts::<Block, voidkin_runtime::RuntimeApi, _>(
        config, telemetry.as_ref().map(|(_, t)| t.handle()), executor,
    )?;
    let client = Arc::new(client);
    let telemetry = telemetry.map(|(worker, telemetry)| { task_manager.spawn_handle().spawn("telemetry", None, worker.run()); telemetry });
    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(), config.role.is_authority().into(),
        config.prometheus_registry(), task_manager.spawn_essential_handle(), client.clone(),
    );

    let (grandpa_block_import, grandpa_link) = sc_consensus_grandpa::block_import(
        client.clone(), 512, &client, select_chain.clone(), telemetry.as_ref().map(|x| x.handle()),
    )?;

    let frontier_backend = open_frontier_backend(client.clone(), config)?;
    let _ = eth_config;

    let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
    let import_queue = sc_consensus_aura::import_queue::<AuraPair, _, _, _, _, _>(sc_consensus_aura::ImportQueueParams {
        block_import: grandpa_block_import.clone(),
        justification_import: Some(Box::new(grandpa_block_import.clone())),
        client: client.clone(),
        create_inherent_data_providers: move |_, ()| async move {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
            let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(*timestamp, slot_duration);
            Ok((slot, timestamp))
        },
        spawner: &task_manager.spawn_essential_handle(),
        registry: config.prometheus_registry(),
        check_for_equivocation: Default::default(),
        telemetry: telemetry.as_ref().map(|x| x.handle()),
        compatibility_mode: Default::default(),
    })?;

    Ok(sc_service::PartialComponents {
        client, backend, task_manager, import_queue, keystore_container, select_chain, transaction_pool,
        other: (telemetry, grandpa_block_import, grandpa_link, frontier_backend),
    })
}

pub fn new_full(mut config: Configuration, eth_config: EthConfiguration) -> Result<TaskManager, ServiceError> {
    let sc_service::PartialComponents {
        client, backend, mut task_manager, import_queue, keystore_container, select_chain,
        transaction_pool, other: (mut telemetry, block_import, grandpa_link, frontier_backend),
    } = new_partial(&config, &eth_config)?;

    let mut net_config = sc_network::config::FullNetworkConfiguration::<Block, <Block as sp_runtime::traits::Block>::Hash, sc_network::NetworkWorker<_, _>>::new(&config.network);
    let metrics = sc_network::NotificationMetrics::new(config.prometheus_registry());
    let peer_store_handle = net_config.peer_store_handle();

    let grandpa_protocol_name = sc_consensus_grandpa::protocol_standard_name(&client.block_hash(0).ok().flatten().expect("Genesis block exists; qed"), &config.chain_spec);
    let (grandpa_protocol_config, grandpa_notification_service) = sc_consensus_grandpa::grandpa_peers_set_config::<_, sc_network::NetworkWorker<_, _>>(grandpa_protocol_name.clone(), metrics.clone(), peer_store_handle);
    net_config.add_notification_protocol(grandpa_protocol_config);

    let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config, net_config, client: client.clone(), transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(), import_queue, block_announce_validator_builder: None,
            warp_sync_config: Some(WarpSyncConfig::WithProvider(Arc::new(sc_consensus_grandpa::warp_proof::NetworkProvider::new(
                backend.clone(), grandpa_link.shared_authority_set().clone(), Vec::default(),
            )))),
            block_relay: None, metrics,
        })?;

    let role = config.role.clone();
    let force_authoring = config.force_authoring;
    let name = config.network.node_name.clone();
    let enable_grandpa = !config.disable_grandpa;
    let prometheus_registry = config.prometheus_registry().cloned();

    // ---- Frontier: mapping sync + caches ----
    let filter_pool: Option<fc_rpc_core::types::FilterPool> = Some(Arc::new(Mutex::new(BTreeMap::new())));
    let fee_history_cache: fc_rpc_core::types::FeeHistoryCache = Arc::new(Mutex::new(BTreeMap::new()));
    let storage_override = Arc::new(fc_storage::StorageOverrideHandler::<Block, _, _>::new(client.clone()));
    let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
        task_manager.spawn_handle(), storage_override.clone(),
        eth_config.eth_log_block_cache, eth_config.eth_statuses_cache, prometheus_registry.clone(),
    ));
    let pubsub_notification_sinks: Arc<fc_mapping_sync::EthereumBlockNotificationSinks<fc_mapping_sync::EthereumBlockNotification<Block>>> = Default::default();

    task_manager.spawn_essential_handle().spawn(
        "frontier-mapping-sync-worker", Some("frontier"),
        fc_mapping_sync::kv::MappingSyncWorker::new(
            client.import_notification_stream(), Duration::new(6, 0), client.clone(), backend.clone(),
            storage_override.clone(), frontier_backend.clone(), 3, 0,
            fc_mapping_sync::SyncStrategy::Normal, sync_service.clone(), pubsub_notification_sinks.clone(),
        ).for_each(|()| futures::future::ready(())),
    );
    task_manager.spawn_essential_handle().spawn(
        "frontier-fee-history", Some("frontier"),
        fc_rpc::EthTask::fee_history_task(client.clone(), storage_override.clone(), fee_history_cache.clone(), eth_config.fee_history_limit),
    );

    // ---- RPC ----
    let rpc_extensions_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();
        let network = network.clone();
        let sync = sync_service.clone();
        let is_authority = role.is_authority();
        let max_past_logs = eth_config.max_past_logs;
        let fee_history_limit = eth_config.fee_history_limit;
        let frontier_backend = frontier_backend.clone();
        Box::new(move |subscription_task_executor: sc_rpc::SubscriptionTaskExecutor| {
            let deps = rpc::FullDeps {
                client: client.clone(), pool: pool.clone(), graph: pool.pool().clone(), is_authority,
                network: network.clone(), sync: sync.clone(), frontier_backend: frontier_backend.clone(),
                storage_override: storage_override.clone(), block_data_cache: block_data_cache.clone(),
                filter_pool: filter_pool.clone(), fee_history_cache: fee_history_cache.clone(),
                fee_history_cache_limit: fee_history_limit, max_past_logs, forced_parent_hashes: None,
                pending_create_inherent_data_providers: Arc::new(|| ()),
            };
            rpc::create_full(deps, subscription_task_executor, pubsub_notification_sinks.clone()).map_err(Into::into)
        })
    };

    let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        network: Arc::new(network.clone()), client: client.clone(), keystore: keystore_container.keystore(),
        task_manager: &mut task_manager, transaction_pool: transaction_pool.clone(),
        rpc_builder: rpc_extensions_builder, backend: backend.clone(), system_rpc_tx, tx_handler_controller,
        sync_service: sync_service.clone(), config, telemetry: telemetry.as_mut(),
    })?;

    // ---- Authoring (Aura) ----
    if role.is_authority() {
        let proposer_factory = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(), client.clone(), transaction_pool.clone(),
            prometheus_registry.as_ref(), telemetry.as_ref().map(|x| x.handle()),
        );
        let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
        let aura = sc_consensus_aura::start_aura::<AuraPair, _, _, _, _, _, _, _, _, _, _>(sc_consensus_aura::StartAuraParams {
            slot_duration, client: client.clone(), select_chain, block_import,
            proposer_factory,
            create_inherent_data_providers: move |_, ()| async move {
                let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
                let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(*timestamp, slot_duration);
                Ok((slot, timestamp))
            },
            force_authoring, backoff_authoring_blocks: Option::<()>::None,
            keystore: keystore_container.keystore(), sync_oracle: sync_service.clone(), justification_sync_link: sync_service.clone(),
            block_proposal_slot_portion: sc_consensus_aura::SlotProportion::new(2f32 / 3f32),
            max_block_proposal_slot_portion: None, telemetry: telemetry.as_ref().map(|x| x.handle()),
            compatibility_mode: Default::default(),
        })?;
        task_manager.spawn_essential_handle().spawn_blocking("aura", Some("block-authoring"), aura);
    }

    // ---- Finality (GRANDPA) ----
    let keystore = if role.is_authority() { Some(keystore_container.keystore()) } else { None };
    if enable_grandpa {
        let grandpa_config = sc_consensus_grandpa::Config {
            gossip_duration: Duration::from_millis(333), justification_generation_period: 512,
            name: Some(name), observer_enabled: false, keystore, local_role: role,
            telemetry: telemetry.as_ref().map(|x| x.handle()), protocol_name: grandpa_protocol_name,
        };
        let grandpa = sc_consensus_grandpa::run_grandpa_voter(sc_consensus_grandpa::GrandpaParams {
            config: grandpa_config, link: grandpa_link, network: network.clone(), sync: Arc::new(sync_service),
            notification_service: grandpa_notification_service, voting_rule: sc_consensus_grandpa::VotingRulesBuilder::default().build(),
            prometheus_registry, shared_voter_state: SharedVoterState::empty(), telemetry: telemetry.as_ref().map(|x| x.handle()),
            offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool),
        })?;
        task_manager.spawn_essential_handle().spawn_blocking("grandpa-voter", None, grandpa);
    }

    network_starter.start_network();
    Ok(task_manager)
}
