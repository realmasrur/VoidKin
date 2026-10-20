//! Runtime APIs: block production, finality, Ethereum RPC support.
use crate::*;
use frame_support::genesis_builder_helper::{build_state, get_preset};
use pallet_grandpa::AuthorityId as GrandpaId;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::crypto::KeyTypeId;

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion { VERSION }
        fn execute_block(block: Block) { Executive::execute_block(block) }
        fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata { OpaqueMetadata::new(Runtime::metadata().into()) }
        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> { Runtime::metadata_at_version(version) }
        fn metadata_versions() -> Vec<u32> { Runtime::metadata_versions() }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }
        fn finalize_block() -> <Block as BlockT>::Header { Executive::finalize_block() }
        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }
        fn check_inherents(block: Block, data: sp_inherents::InherentData) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(source: TransactionSource, tx: <Block as BlockT>::Extrinsic, block_hash: <Block as BlockT>::Hash) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) { Executive::offchain_worker(header) }
    }

    impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
        fn slot_duration() -> sp_consensus_aura::SlotDuration {
            sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
        }
        fn authorities() -> Vec<AuraId> { pallet_aura::Authorities::<Runtime>::get().into_inner() }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> { opaque::SessionKeys::generate(seed) }
        fn decode_session_keys(encoded: Vec<u8>) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> sp_consensus_grandpa::AuthorityList { Grandpa::grandpa_authorities() }
        fn current_set_id() -> sp_consensus_grandpa::SetId { Grandpa::current_set_id() }
        fn submit_report_equivocation_unsigned_extrinsic(_p: sp_consensus_grandpa::EquivocationProof<<Block as BlockT>::Hash, NumberFor<Block>>, _k: sp_consensus_grandpa::OpaqueKeyOwnershipProof) -> Option<()> { None }
        fn generate_key_ownership_proof(_s: sp_consensus_grandpa::SetId, _a: GrandpaId) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> { None }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce { System::account_nonce(account) }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result { build_state::<RuntimeGenesisConfig>(config) }
        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            get_preset::<RuntimeGenesisConfig>(id, crate::genesis::get_preset)
        }
        fn preset_names() -> Vec<sp_genesis_builder::PresetId> { crate::genesis::preset_names() }
    }

    impl fp_rpc::EthereumRuntimeRPCApi<Block> for Runtime {
        fn chain_id() -> u64 { <Runtime as pallet_evm::Config>::ChainId::get() }
        fn account_basic(address: H160) -> EVMAccount { let (account, _) = pallet_evm::Pallet::<Runtime>::account_basic(&address); account }
        fn gas_price() -> U256 { let (p, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price(); p }
        fn account_code_at(address: H160) -> Vec<u8> { pallet_evm::AccountCodes::<Runtime>::get(address) }
        fn author() -> H160 { <pallet_evm::Pallet<Runtime>>::find_author() }
        fn storage_at(address: H160, index: U256) -> H256 {
            let mut tmp = [0u8; 32];
            index.to_big_endian(&mut tmp);
            pallet_evm::AccountStorages::<Runtime>::get(address, H256::from_slice(&tmp[..]))
        }
        fn current_block() -> Option<pallet_ethereum::Block> { pallet_ethereum::CurrentBlock::<Runtime>::get() }
        fn current_receipts() -> Option<Vec<pallet_ethereum::Receipt>> { pallet_ethereum::CurrentReceipts::<Runtime>::get() }
        fn current_transaction_statuses() -> Option<Vec<fp_rpc::TransactionStatus>> { pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get() }
        fn current_all() -> (Option<pallet_ethereum::Block>, Option<Vec<pallet_ethereum::Receipt>>, Option<Vec<fp_rpc::TransactionStatus>>) {
            (pallet_ethereum::CurrentBlock::<Runtime>::get(), pallet_ethereum::CurrentReceipts::<Runtime>::get(), pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get())
        }
        fn extrinsic_filter(xts: Vec<<Block as BlockT>::Extrinsic>) -> Vec<EthereumTransaction> {
            xts.into_iter().filter_map(|xt| match xt.0.function {
                RuntimeCall::Ethereum(transact { transaction }) => Some(transaction), _ => None }).collect()
        }
        fn elasticity() -> Option<Permill> { Some(pallet_base_fee::Elasticity::<Runtime>::get()) }
        fn gas_limit_multiplier_support() {}
        fn pending_block(_xts: Vec<<Block as BlockT>::Extrinsic>) -> (Option<pallet_ethereum::Block>, Option<Vec<fp_rpc::TransactionStatus>>) { (None, None) }
        fn initialize_pending_block(header: &<Block as BlockT>::Header) { Executive::initialize_block(header); }
        // call / create / etc. are provided by the Frontier template; copy them from
        // frontier/template/runtime/src/lib.rs (they follow the pallet_evm Runner pattern).
    }

    impl fp_rpc::ConvertTransactionRuntimeApi<Block> for Runtime {
        fn convert_transaction(transaction: EthereumTransaction) -> <Block as BlockT>::Extrinsic {
            UncheckedExtrinsic::new_unsigned(pallet_ethereum::Call::<Runtime>::transact { transaction }.into())
        }
    }
}
