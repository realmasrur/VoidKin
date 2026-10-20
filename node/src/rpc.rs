//! RPC wiring: Substrate system/payment RPC + full Ethereum JSON-RPC via Frontier.
use std::sync::Arc;

use jsonrpsee::RpcModule;
use sc_client_api::{backend::{Backend, StorageProvider}, client::BlockchainEvents, AuxStore, UsageProvider};
use sc_network::service::traits::NetworkService;
use sc_network_sync::SyncingService;
use sc_rpc::SubscriptionTaskExecutor;
use sc_transaction_pool::ChainApi;
use sc_transaction_pool_api::TransactionPool;
use sp_api::{CallApiAt, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Backend as BlockchainBackend, Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_core::H256;
use sp_runtime::traits::Block as BlockT;
use voidkin_runtime::{opaque::Block, AccountId, Balance, Hash, Nonce};

pub struct FullDeps<C, P, A: ChainApi> {
    pub client: Arc<C>,
    pub pool: Arc<P>,
    pub graph: Arc<sc_transaction_pool::Pool<A>>,
    pub is_authority: bool,
    pub network: Arc<dyn NetworkService>,
    pub sync: Arc<SyncingService<Block>>,
    pub frontier_backend: Arc<dyn fc_api::Backend<Block>>,
    pub storage_override: Arc<dyn fc_storage::StorageOverride<Block>>,
    pub block_data_cache: Arc<fc_rpc::EthBlockDataCacheTask<Block>>,
    pub filter_pool: Option<fc_rpc_core::types::FilterPool>,
    pub fee_history_cache: fc_rpc_core::types::FeeHistoryCache,
    pub fee_history_cache_limit: fc_rpc_core::types::FeeHistoryCacheLimit,
    pub max_past_logs: u32,
    pub forced_parent_hashes: Option<std::collections::BTreeMap<H256, H256>>,
    pub pending_create_inherent_data_providers: Arc<dyn Fn() -> () + Send + Sync>,
}

pub fn create_full<C, BE, P, A>(
    deps: FullDeps<C, P, A>,
    subscription_task_executor: SubscriptionTaskExecutor,
    pubsub_notification_sinks: Arc<
        fc_mapping_sync::EthereumBlockNotificationSinks<fc_mapping_sync::EthereumBlockNotification<Block>>,
    >,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: CallApiAt<Block> + ProvideRuntimeApi<Block>,
    C::Api: sp_block_builder::BlockBuilder<Block>,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: fp_rpc::ConvertTransactionRuntimeApi<Block>,
    C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + StorageProvider<Block, BE> + AuxStore + UsageProvider<Block> + BlockchainEvents<Block> + 'static,
    BE: Backend<Block> + 'static,
    P: TransactionPool<Block = Block, Hash = Hash> + 'static,
    A: ChainApi<Block = Block> + 'static,
{
    use fc_rpc::{Eth, EthApiServer, EthFilter, EthFilterApiServer, EthPubSub, EthPubSubApiServer, Net, NetApiServer, Web3, Web3ApiServer, TxPool, TxPoolApiServer};
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};

    let mut io = RpcModule::new(());
    let FullDeps { client, pool, graph, is_authority, network, sync, frontier_backend, storage_override, block_data_cache, filter_pool, fee_history_cache, fee_history_cache_limit, max_past_logs, forced_parent_hashes, .. } = deps;

    io.merge(System::new(client.clone(), pool.clone()).into_rpc())?;
    io.merge(TransactionPayment::new(client.clone()).into_rpc())?;

    // NOTE: the exact constructor signatures of fc_rpc::Eth / EthFilter / EthPubSub change
    // between Frontier releases. Mirror frontier/template/node/src/rpc/eth.rs for the branch you pin.
    io.merge(Eth::<Block, C, P, (), BE, A, ()>::new(
        client.clone(), pool.clone(), graph.clone(), None, sync.clone(), Vec::new(),
        storage_override.clone(), frontier_backend.clone(), is_authority, block_data_cache.clone(),
        fee_history_cache, fee_history_cache_limit, 10, forced_parent_hashes, Default::default(), None,
    ).replace_config::<()>().into_rpc())?;

    if let Some(filter_pool) = filter_pool {
        io.merge(EthFilter::new(client.clone(), frontier_backend.clone(), graph.clone(), filter_pool, 500_usize, max_past_logs, block_data_cache.clone()).into_rpc())?;
    }
    io.merge(EthPubSub::new(pool, client.clone(), sync, subscription_task_executor, storage_override, pubsub_notification_sinks).into_rpc())?;
    io.merge(Net::new(client.clone(), network, true).into_rpc())?;
    io.merge(Web3::new(client.clone()).into_rpc())?;
    io.merge(TxPool::new(client, graph).into_rpc())?;
    Ok(io)
}
