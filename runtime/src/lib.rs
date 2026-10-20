//! VOIDKIN runtime: Substrate + Frontier (full Ethereum / EVM compatibility).
//! Consensus: Aura (block production) + GRANDPA (finality), PoA at launch.
//! Native token: KIN (18 decimals, Ethereum-style H160 accounts).
#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

extern crate alloc;
use alloc::{vec, vec::Vec, borrow::Cow};
use core::marker::PhantomData;

use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstBool, ConstU32, ConstU64, ConstU128, FindAuthor, OnFinalize},
    weights::{constants::WEIGHT_REF_TIME_PER_SECOND, IdentityFee, Weight},
    runtime,
};
use pallet_ethereum::{Call::transact, PostLogContent, Transaction as EthereumTransaction, TransactionAction, TransactionData};
use pallet_evm::{Account as EVMAccount, EnsureAccountId20, FeeCalculator, IdentityAddressMapping, Runner};
use fp_account::EthereumSignature;
use sp_api::impl_runtime_apis;
use sp_core::{crypto::ByteArray, H160, H256, U256, OpaqueMetadata};
use sp_runtime::{
    generic, impl_opaque_keys,
    traits::{BlakeTwo256, Block as BlockT, DispatchInfoOf, Dispatchable, IdentifyAccount, NumberFor, PostDispatchInfoOf, UniqueSaturatedInto, Verify},
    transaction_validity::{TransactionSource, TransactionValidity, TransactionValidityError},
    ApplyExtrinsicResult, ConsensusEngineId, Perbill, Permill,
};
use sp_version::RuntimeVersion;

pub use frame_system::Call as SystemCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;

// ---------- Primitives ----------
pub type Signature = EthereumSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId; // H160
pub type Balance = u128;
pub type Nonce = u32;
pub type Hash = H256;
pub type BlockNumber = u32;
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

pub mod opaque {
    use super::*;
    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    impl_opaque_keys! {
        pub struct SessionKeys {
            pub aura: Aura,
            pub grandpa: Grandpa,
        }
    }
}

// ---------- Chain constants ----------
pub const MILLI_SECS_PER_BLOCK: u64 = 6000; // 6s block time
pub const SLOT_DURATION: u64 = MILLI_SECS_PER_BLOCK;
pub const MINUTES: BlockNumber = 60_000 / (MILLI_SECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

pub const KIN: Balance = 1_000_000_000_000_000_000; // 18 decimals
pub const MILLIKIN: Balance = KIN / 1_000;
pub const EXISTENTIAL_DEPOSIT: Balance = MILLIKIN;

/// PLACEHOLDER chain id: replace with your final chain id before public testnet.
pub const CHAIN_ID: u64 = 7777;

/// Gas: 15M per 6s block (Frontier default style).
pub const BLOCK_GAS_LIMIT: u64 = 15_000_000;
pub const MAX_POV_SIZE: u64 = 5 * 1024 * 1024;
pub const WEIGHT_PER_GAS: u64 = 20_000;

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: Cow::Borrowed("voidkin"),
    impl_name: Cow::Borrowed("voidkin"),
    authoring_version: 1,
    spec_version: 1,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    system_version: 1,
};

#[cfg(feature = "std")]
pub fn native_version() -> sp_version::NativeVersion {
    sp_version::NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

parameter_types! {
    pub const Version: RuntimeVersion = VERSION;
    pub const BlockHashCount: BlockNumber = 2400;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::with_sensible_defaults(
            Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_SECOND, u64::MAX),
            Perbill::from_percent(75),
        );
    pub BlockLength: frame_system::limits::BlockLength =
        frame_system::limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, Perbill::from_percent(75));
    pub const SS58Prefix: u16 = 42;
}

// ---------- Runtime composition ----------
#[runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(RuntimeCall, RuntimeEvent, RuntimeError, RuntimeOrigin, RuntimeFreezeReason, RuntimeHoldReason, RuntimeSlashReason, RuntimeLockId, RuntimeTask)]
    pub struct Runtime;

    #[runtime::pallet_index(0)] pub type System = frame_system;
    #[runtime::pallet_index(1)] pub type Timestamp = pallet_timestamp;
    #[runtime::pallet_index(2)] pub type Aura = pallet_aura;
    #[runtime::pallet_index(3)] pub type Grandpa = pallet_grandpa;
    #[runtime::pallet_index(4)] pub type Balances = pallet_balances;
    #[runtime::pallet_index(5)] pub type TransactionPayment = pallet_transaction_payment;
    #[runtime::pallet_index(6)] pub type Sudo = pallet_sudo;
    // Ethereum compatibility
    #[runtime::pallet_index(7)] pub type Ethereum = pallet_ethereum;
    #[runtime::pallet_index(8)] pub type EVM = pallet_evm;
    #[runtime::pallet_index(9)] pub type EVMChainId = pallet_evm_chain_id;
    #[runtime::pallet_index(10)] pub type BaseFee = pallet_base_fee;
}

#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
    type BlockWeights = BlockWeights;
    type BlockLength = BlockLength;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
    type Nonce = Nonce;
    type Block = Block;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type BlockHashCount = BlockHashCount;
    type Version = Version;
    type AccountData = pallet_balances::AccountData<Balance>;
    type SS58Prefix = SS58Prefix;
}
use sp_runtime::traits::IdentityLookup;

impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = Aura;
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = ();
}

impl pallet_aura::Config for Runtime {
    type AuthorityId = sp_consensus_aura::sr25519::AuthorityId;
    type DisabledValidators = ();
    type MaxAuthorities = ConstU32<32>;
    type AllowMultipleBlocksPerSlot = ConstBool<false>;
    type SlotDuration = pallet_aura::MinimumPeriodTimesTwo<Runtime>;
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = ConstU32<32>;
    type MaxNominators = ConstU32<0>;
    type MaxSetIdSessionEntries = ConstU64<0>;
    type KeyOwnerProof = sp_core::Void;
    type EquivocationReportSystem = ();
}

impl pallet_balances::Config for Runtime {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type DoneSlashHandler = ();
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ();
}
use frame_support::traits::ConstU8;

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

// ---------- Frontier / EVM ----------
impl pallet_evm_chain_id::Config for Runtime {}

pub struct FindAuthorTruncated<F>(PhantomData<F>);
impl<F: FindAuthor<u32>> FindAuthor<H160> for FindAuthorTruncated<F> {
    fn find_author<'a, I>(digests: I) -> Option<H160>
    where I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])> {
        // Block rewards/tips go to the validator's account derived from its Aura key.
        F::find_author(digests).and_then(|i| {
            Aura::authorities().get(i as usize).map(|id| H160::from_slice(&id.to_raw_vec()[4..24]))
        })
    }
}

parameter_types! {
    pub BlockGasLimit: U256 = U256::from(BLOCK_GAS_LIMIT);
    pub const GasLimitPovSizeRatio: u64 = BLOCK_GAS_LIMIT.saturating_div(MAX_POV_SIZE);
    pub const GasLimitStorageGrowthRatio: u64 = 0;
    pub PrecompilesValue: () = ();
    pub WeightPerGas: Weight = Weight::from_parts(WEIGHT_PER_GAS, 0);
    pub const PostBlockAndTxnHashes: PostLogContent = PostLogContent::BlockAndTxnHashes;
}

impl pallet_evm::Config for Runtime {
    type FeeCalculator = BaseFee;
    type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
    type WeightPerGas = WeightPerGas;
    type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
    type CallOrigin = EnsureAccountId20;
    type WithdrawOrigin = EnsureAccountId20;
    type AddressMapping = IdentityAddressMapping;
    type Currency = Balances; // KIN is the EVM gas / value currency
    type RuntimeEvent = RuntimeEvent;
    type PrecompilesType = ();
    type PrecompilesValue = PrecompilesValue;
    type ChainId = EVMChainId;
    type BlockGasLimit = BlockGasLimit;
    type Runner = pallet_evm::runner::stack::Runner<Self>;
    type OnChargeTransaction = ();
    type OnCreate = ();
    type FindAuthor = FindAuthorTruncated<Aura>;
    type GasLimitPovSizeRatio = GasLimitPovSizeRatio;
    type GasLimitStorageGrowthRatio = GasLimitStorageGrowthRatio;
    type Timestamp = Timestamp;
    type CreateOriginFilter = ();
    type CreateInnerOriginFilter = ();
    type WeightInfo = pallet_evm::weights::SubstrateWeight<Self>;
}

impl pallet_ethereum::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
    type PostLogContent = PostBlockAndTxnHashes;
    type ExtraDataLength = ConstU32<30>;
}

parameter_types! {
    pub DefaultBaseFeePerGas: U256 = U256::from(1_000_000_000u64); // 1 gwei-KIN start
    pub DefaultElasticity: Permill = Permill::from_parts(125_000);
}
pub struct BaseFeeThreshold;
impl pallet_base_fee::BaseFeeThreshold for BaseFeeThreshold {
    fn lower() -> Permill { Permill::zero() }
    fn ideal() -> Permill { Permill::from_parts(500_000) }
    fn upper() -> Permill { Permill::from_parts(1_000_000) }
}
impl pallet_base_fee::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Threshold = BaseFeeThreshold;
    type DefaultBaseFeePerGas = DefaultBaseFeePerGas;
    type DefaultElasticity = DefaultElasticity;
}

// ---------- Extrinsic plumbing (self-contained Ethereum txs) ----------
impl fp_self_contained::SelfContainedCall for RuntimeCall {
    type SignedInfo = H160;
    fn is_self_contained(&self) -> bool {
        match self { RuntimeCall::Ethereum(call) => call.is_self_contained(), _ => false }
    }
    fn check_self_contained(&self) -> Option<Result<Self::SignedInfo, TransactionValidityError>> {
        match self { RuntimeCall::Ethereum(call) => call.check_self_contained(), _ => None }
    }
    fn validate_self_contained(&self, info: &Self::SignedInfo, dispatch_info: &DispatchInfoOf<RuntimeCall>, len: usize) -> Option<TransactionValidity> {
        match self { RuntimeCall::Ethereum(call) => call.validate_self_contained(info, dispatch_info, len), _ => None }
    }
    fn pre_dispatch_self_contained(&self, info: &Self::SignedInfo, dispatch_info: &DispatchInfoOf<RuntimeCall>, len: usize) -> Option<Result<(), TransactionValidityError>> {
        match self { RuntimeCall::Ethereum(call) => call.pre_dispatch_self_contained(info, dispatch_info, len), _ => None }
    }
    fn apply_self_contained(self, info: Self::SignedInfo) -> Option<sp_runtime::DispatchResultWithInfo<PostDispatchInfoOf<Self>>> {
        match self {
            call @ RuntimeCall::Ethereum(pallet_ethereum::Call::transact { .. }) =>
                Some(call.dispatch(RuntimeOrigin::from(pallet_ethereum::RawOrigin::EthereumTransaction(info)))),
            _ => None,
        }
    }
}

pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);
pub type UncheckedExtrinsic = fp_self_contained::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
pub type Address = AccountId;
pub type Executive = frame_executive::Executive<Runtime, Block, frame_system::ChainContext<Runtime>, Runtime, AllPalletsWithSystem>;

pub mod genesis;
mod apis;
// Runtime APIs (Aura, GRANDPA, system, payment, Ethereum RPC) are implemented in
// runtime/src/apis.rs. See README "Next step".
