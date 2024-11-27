#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// // Make the WASM binary available.
// #[cfg(feature = "std")]
// include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;

use codec::Encode;
pub use dia_oracle::dia::*;
use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, ConstU8, Contains},
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, ConstantMultiplier, IdentityFee, Weight},
	PalletId,
	genesis_builder_helper::{create_default_config, build_config},

};
pub use frame_system::Call as SystemCall;
use orml_currencies::BasicCurrencyAdapter;
use orml_traits::{currency::MutationHooks, parameter_type_with_key};
pub use pallet_balances::Call as BalancesCall;
use pallet_grandpa::{
	fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList,
};
pub use pallet_timestamp::Call as TimestampCall;
use sp_api::impl_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{OpaqueMetadata, H256};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, AccountIdLookup, BlakeTwo256, Block as BlockT, ConstBool, Convert,
		NumberFor, Zero,
	},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, DispatchError, FixedPointNumber, Perbill, Perquintill,
	SaturatedConversion,
};
use sp_std::{marker::PhantomData, prelude::*};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use currency::Amount;
pub use issue::{Event as IssueEvent, IssueRequest};
pub use module_oracle_rpc_runtime_api::BalanceWrapper;
pub use nomination::Event as NominationEvent;
use oracle::dia::{DiaOracleAdapter, NativeCurrencyKey, XCMCurrencyConversion};
pub use primitives::{
	self, AccountId, Balance, BlockNumber, CurrencyId, Hash, Moment, Nonce, Signature,
	SignedFixedPoint, SignedInner, UnsignedFixedPoint, UnsignedInner,
};
pub use redeem::{Event as RedeemEvent, RedeemRequest};
pub use replace::{Event as ReplaceEvent, ReplaceRequest};
pub use security::StatusCode;
pub use stellar_relay::traits::{FieldLength, Organization, Validator};

type VaultId = primitives::VaultId<AccountId, CurrencyId>;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
		pub grandpa: Grandpa,
	}
}

pub const UNITS: Balance = 10_000_000_000;
pub const CENTS: Balance = UNITS / 100;
// 100_000_000
pub const MILLICENTS: Balance = CENTS / 1_000; // 100_000

// These time units are defined in number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;
pub const WEEKS: BlockNumber = DAYS * 7;
pub const YEARS: BlockNumber = DAYS * 365;

/// This runtime version.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("spacewalk-standalone"),
	impl_name: create_runtime_str!("spacewalk-standalone"),
	authoring_version: 1,
	spec_version: 1,
	impl_version: 1,
	transaction_version: 1,
	apis: RUNTIME_API_VERSIONS,
	state_version: 0,
};

pub const MILLISECS_PER_BLOCK: u64 = 6000;
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

pub struct BlockNumberToBalance;

impl Convert<BlockNumber, Balance> for BlockNumberToBalance {
	fn convert(a: BlockNumber) -> Balance {
		a.into()
	}
}

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;
	pub const BlockHashCount: BlockNumber = 250;
	/// We allow for 2 seconds of compute with a 6 second average block time.
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::with_sensible_defaults(
			(2u64 * Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND,0)).set_proof_size(u64::MAX),
			NORMAL_DISPATCH_RATIO,
		);
	pub BlockLength: frame_system::limits::BlockLength = frame_system::limits::BlockLength
		::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Runtime {
	type Block = Block;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = BlockWeights;
	type BlockLength = BlockLength;
	/// The ubiquitous origin type.
	type RuntimeOrigin = RuntimeOrigin;
	/// The aggregated dispatch type that is available for extrinsics.
	type RuntimeCall = RuntimeCall;
	/// The index type for storing how many extrinsics an account has signed.
	type Nonce = Nonce;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = AccountIdLookup<AccountId, ()>;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	/// Runtime version.
	type Version = Version;
	/// Converts a module to an index of this module in the runtime.
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type RuntimeTask = RuntimeTask;
}

parameter_types! {
	pub const MaxAuthorities: u32 = 32;
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = MaxAuthorities;
	type AllowMultipleBlocksPerSlot = ConstBool<false>;
}

impl pallet_grandpa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type KeyOwnerProof = sp_core::Void;
	type WeightInfo = ();
	type MaxAuthorities = MaxAuthorities;
	type MaxSetIdSessionEntries = ConstU64<0>;
	type EquivocationReportSystem = ();
	type MaxNominators = ConstU32<1000>;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = Moment;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

const NATIVE_CURRENCY_ID: CurrencyId = CurrencyId::Native;
const PARENT_CURRENCY_ID: CurrencyId = CurrencyId::XCM(0);
#[cfg(feature = "runtime-benchmarks")]
const WRAPPED_CURRENCY_ID: CurrencyId = currency::testing_constants::DEFAULT_WRAPPED_CURRENCY;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = NATIVE_CURRENCY_ID;
	pub const GetRelayChainCurrencyId: CurrencyId = PARENT_CURRENCY_ID;
	pub const TransactionByteFee: Balance = MILLICENTS;
}

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
	pub const GetWrappedCurrencyId: CurrencyId = WRAPPED_CURRENCY_ID;
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = pallet_transaction_payment::CurrencyAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = IdentityFee<Balance>;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = ();
}

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = ();
}

// Pallet accounts
parameter_types! {
	pub const FeePalletId: PalletId = PalletId(*b"mod/fees");
	pub const VaultRegistryPalletId: PalletId = PalletId(*b"mod/vreg");
}

parameter_types! {
	// 5EYCAe5i8QbRr5WN1PvaAVqPbfXsqazk9ocaxuzcTjgXPM1e
	pub FeeAccount: AccountId = FeePalletId::get().into_account_truncating();
	// 5EYCAe5i8QbRra1jndPz1WAuf1q1KHQNfu2cW1EXJ231emTd
	pub VaultRegistryAccount: AccountId = VaultRegistryPalletId::get().into_account_truncating();
}

pub fn get_all_module_accounts() -> Vec<AccountId> {
	vec![FeeAccount::get(), VaultRegistryAccount::get()]
}

parameter_types! {
	pub const MaxLocks: u32 = 50;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Zero::zero()
	};
}

pub struct CurrencyHooks<T>(PhantomData<T>);

impl<T: orml_tokens::Config> MutationHooks<T::AccountId, T::CurrencyId, T::Balance>
	for CurrencyHooks<T>
{
	type OnDust = orml_tokens::BurnDust<T>;
	type OnSlash = ();
	type PreDeposit = ();
	type PostDeposit = ();
	type PreTransfer = ();
	type PostTransfer = ();
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = primitives::Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = CurrencyHooks<Runtime>;
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = DustRemovalWhitelist;
}

pub struct DustRemovalWhitelist;

impl Contains<AccountId> for DustRemovalWhitelist {
	fn contains(a: &AccountId) -> bool {
		[].contains(a)
	}
}

impl orml_currencies::Config for Runtime {
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, primitives::Amount, BlockNumber>;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}

/// Existential deposit.
pub const EXISTENTIAL_DEPOSIT: u128 = 500;

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ConstU32<1>;
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
}

impl security::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = security::SubstrateWeight<Runtime>;
}

pub struct CurrencyConvert;

impl currency::CurrencyConversion<currency::Amount<Runtime>, CurrencyId> for CurrencyConvert {
	fn convert(
		amount: &currency::Amount<Runtime>,
		to: CurrencyId,
	) -> Result<currency::Amount<Runtime>, DispatchError> {
		Oracle::convert(amount, to)
	}
}

impl currency::Config for Runtime {
	type SignedInner = SignedInner;
	type SignedFixedPoint = SignedFixedPoint;
	type UnsignedFixedPoint = UnsignedFixedPoint;
	type Balance = Balance;
	type GetRelayChainCurrencyId = GetRelayChainCurrencyId;
	#[cfg(feature = "runtime-benchmarks")]
	type GetWrappedCurrencyId = GetWrappedCurrencyId;
	type AssetConversion = primitives::AssetConversion;
	type BalanceConversion = primitives::BalanceConversion;
	type CurrencyConversion = CurrencyConvert;
	type AmountCompatibility = primitives::StellarCompatibility;
}

impl staking::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type CurrencyId = CurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type MaxRewardCurrencies = MaxRewardCurrencies;
}

pub type OrganizationId = u128;

parameter_types! {
	pub const OrganizationLimit: u32 = 255;
	pub const ValidatorLimit: u32 = 255;
	pub const IsPublicNetwork: bool = true;
}

impl stellar_relay::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OrganizationId = OrganizationId;
	type OrganizationLimit = OrganizationLimit;
	type ValidatorLimit = ValidatorLimit;
	type IsPublicNetwork = IsPublicNetwork;
	type WeightInfo = stellar_relay::SubstrateWeight<Runtime>;
}

impl vault_registry::Config for Runtime {
	type PalletId = VaultRegistryPalletId;
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type WeightInfo = vault_registry::SubstrateWeight<Runtime>;
	type GetGriefingCollateralCurrencyId = GetRelayChainCurrencyId;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = UncheckedExtrinsic;
}

impl dia_oracle::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type AuthorityId = dia_oracle::crypto::DiaAuthId;
	type WeightInfo = dia_oracle::weights::DiaWeightInfo<Runtime>;
}

impl frame_system::offchain::SigningTypes for Runtime {
	type Public = <Signature as sp_runtime::traits::Verify>::Signer;
	type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
	RuntimeCall: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: RuntimeCall,
		public: <Signature as sp_runtime::traits::Verify>::Signer,
		account: AccountId,
		index: Nonce,
	) -> Option<(
		RuntimeCall,
		<UncheckedExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
	)> {
		let period = BlockHashCount::get() as u64;
		let current_block = System::block_number().saturated_into::<u64>().saturating_sub(1);
		let tip = 0;
		let extra: SignedExtra = (
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
			frame_system::CheckNonce::<Runtime>::from(index),
			frame_system::CheckWeight::<Runtime>::new(),
			pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
		);

		let raw_payload = SignedPayload::new(call, extra).ok()?;
		let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
		let address = account;
		let (call, extra, _) = raw_payload.deconstruct();
		Some((call, (sp_runtime::MultiAddress::Id(address), signature, extra)))
	}
}

pub struct ConvertPrice;

impl Convert<u128, Option<UnsignedFixedPoint>> for ConvertPrice {
	fn convert(price: u128) -> Option<UnsignedFixedPoint> {
		Some(UnsignedFixedPoint::from_inner(price))
	}
}

pub struct ConvertMoment;

impl Convert<u64, Option<Moment>> for ConvertMoment {
	fn convert(moment: u64) -> Option<Moment> {
		// The provided moment is in seconds, but we need milliseconds
		Some(moment.saturating_mul(1000))
	}
}

pub struct SpacewalkNativeCurrencyKey;

impl NativeCurrencyKey for SpacewalkNativeCurrencyKey {
	fn native_symbol() -> Vec<u8> {
		"LOCAL".as_bytes().to_vec()
	}

	fn native_chain() -> Vec<u8> {
		"LOCAL".as_bytes().to_vec()
	}
}

// It's important that this is implemented the same way as the MockOracleKeyConvertor
// because this is used in the benchmark_utils::DataCollector when feeding prices
impl XCMCurrencyConversion for SpacewalkNativeCurrencyKey {
	fn convert_to_dia_currency_id(token_symbol: u8) -> Option<(Vec<u8>, Vec<u8>)> {
		// todo: this code results in Execution error:
		// todo: \"Unable to get required collateral for amount\":
		// todo: Module(ModuleError { index: 19, error: [0, 0, 0, 0], message: None })", data: None
		// } cfg_if::cfg_if! {
		// 	if #[cfg(not(feature = "testing-utils"))] {
		// 		if token_symbol == 0 {
		// 			return Some((b"Kusama".to_vec(), b"KSM".to_vec()))
		// 		}
		// 	}
		// }
		// We assume that the blockchain is always 0 and the symbol represents the token symbol
		let blockchain = vec![0u8];
		let symbol = vec![token_symbol];

		Some((blockchain, symbol))
	}

	fn convert_from_dia_currency_id(blockchain: Vec<u8>, symbol: Vec<u8>) -> Option<u8> {
		// We assume that the blockchain is always 0 and the symbol represents the token symbol
		if blockchain.len() != 1 && blockchain[0] != 0 || symbol.len() != 1 {
			return None;
		}
		Some(symbol[0])
	}
}

cfg_if::cfg_if! {
	 if #[cfg(feature = "testing-utils")] {
		type DataProviderImpl = DiaOracleAdapter<
			DiaOracleModule,
			UnsignedFixedPoint,
			Moment,
			oracle::dia::DiaOracleKeyConvertor<SpacewalkNativeCurrencyKey>,
			ConvertPrice,
			ConvertMoment,
		>;
	} else if #[cfg(feature = "runtime-benchmarks")] {
		use oracle::testing_utils::{
			MockConvertMoment, MockConvertPrice, MockDiaOracle, MockOracleKeyConvertor,
		};
		type DataProviderImpl = DiaOracleAdapter<
			MockDiaOracle,
			UnsignedFixedPoint,
			Moment,
			MockOracleKeyConvertor,
			MockConvertPrice,
			MockConvertMoment<Moment>,
		>;
	} else {
		// This implementation will be used when running the testchain locally
		// as well as for the **vault integration tests**
		type DataProviderImpl = DiaOracleAdapter<
			DiaOracleModule,
			UnsignedFixedPoint,
			Moment,
			oracle::dia::DiaOracleKeyConvertor<SpacewalkNativeCurrencyKey>,
			ConvertPrice,
			ConvertMoment,
		>;
	}
}

#[cfg(any(feature = "runtime-benchmarks", feature = "testing-utils"))]
use oracle::testing_utils::MockDataFeeder;

impl oracle::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = oracle::SubstrateWeight<Runtime>;
	type DecimalsLookup = primitives::DefaultDecimalsLookup;
	type DataProvider = DataProviderImpl;

	#[cfg(any(feature = "runtime-benchmarks", feature = "testing-utils"))]
	type DataFeeder = MockDataFeeder<AccountId, Moment>;
}

impl issue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BlockNumberToBalance = BlockNumberToBalance;
	type WeightInfo = issue::SubstrateWeight<Runtime>;
}

impl redeem::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = redeem::SubstrateWeight<Runtime>;
}

impl replace::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = replace::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const MaxExpectedValue: UnsignedFixedPoint = UnsignedFixedPoint::from_inner(<UnsignedFixedPoint as FixedPointNumber>::DIV);
}

impl fee::Config for Runtime {
	type FeePalletId = FeePalletId;
	type WeightInfo = fee::SubstrateWeight<Runtime>;
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type UnsignedFixedPoint = UnsignedFixedPoint;
	type UnsignedInner = UnsignedInner;
	type VaultRewards = VaultRewards;
	type VaultStaking = VaultStaking;
	type OnSweep = currency::SweepFunds<Runtime, FeeAccount>;
	type MaxExpectedValue = MaxExpectedValue;
	type RewardDistribution = RewardDistribution;
}

impl nomination::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = nomination::SubstrateWeight<Runtime>;
}

impl clients_info::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = clients_info::SubstrateWeight<Runtime>;
	type MaxNameLength = ConstU32<255>;
	type MaxUriLength = ConstU32<255>;
}

parameter_types! {
	pub const DecayRate: Perquintill = Perquintill::from_percent(5);
	pub const MaxCurrencies: u32 = 10;
}

impl reward_distribution::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = reward_distribution::SubstrateWeight<Runtime>;
	type Balance = Balance;
	type DecayInterval = ConstU32<100>;
	type DecayRate = DecayRate;
	type VaultRewards = VaultRewards;
	type MaxCurrencies = MaxCurrencies;
	type OracleApi = Oracle;
	type Balances = Balances;
	type VaultStaking = VaultStaking;
	type FeePalletId = FeePalletId;
}

parameter_types! {
	pub const MaxRewardCurrencies: u32= 10;
}

impl pooled_rewards::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type PoolId = CurrencyId;
	type PoolRewardsCurrencyId = CurrencyId;
	type StakeId = VaultId;
	type MaxRewardCurrencies = MaxRewardCurrencies;
}

construct_runtime! {
	pub enum Runtime
	{
		System: frame_system = 0,
		Timestamp: pallet_timestamp = 1,
		Aura: pallet_aura = 2,
		Grandpa: pallet_grandpa = 3,
		Sudo: pallet_sudo = 4,
		Tokens: orml_tokens = 5,
		Currencies: orml_currencies = 7,
		Balances: pallet_balances = 8,
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>} = 9,

		StellarRelay: stellar_relay = 10,

		VaultRewards: pooled_rewards = 15,
		VaultStaking: staking = 16,

		Currency: currency = 17,

		Security: security = 19,
		VaultRegistry: vault_registry = 21,
		Oracle: oracle = 22,
		Issue: issue = 23,
		Redeem: redeem = 24,
		Replace: replace = 25,
		Fee: fee = 26,
		Nomination: nomination = 28,
		DiaOracleModule: dia_oracle = 29,
		ClientsInfo: clients_info = 30,
		RewardDistribution: reward_distribution = 31,
	}
}

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	define_benchmarks!(
		[clients_info, ClientsInfo]
		[frame_benchmarking, BaselineBench::<Runtime>]
		[frame_system, SystemBench::<Runtime>]
		[stellar_relay, StellarRelay]
		[issue, Issue]
		[fee, Fee]
		[oracle, Oracle]
		[redeem, Redeem]
		[replace, Replace]
		[vault_registry, VaultRegistry]
		[nomination, Nomination]
		[reward_distribution, RewardDistribution]
	);
}

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}

		fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
			Runtime::metadata_at_version(version)
		}

		fn metadata_versions() -> sp_std::vec::Vec<u32> {
			Runtime::metadata_versions()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}

		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}

		fn current_set_id() -> fg_primitives::SetId {
			Grandpa::current_set_id()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			_equivocation_proof: fg_primitives::EquivocationProof<
				<Block as BlockT>::Hash,
				NumberFor<Block>,
			>,
			_key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			None
		}

		fn generate_key_ownership_proof(
			_set_id: fg_primitives::SetId,
			_authority_id: GrandpaId,
		) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
			// NOTE: this is the only implementation possible since we've
			// defined our key owner proof type as a bottom type (i.e. a type
			// with no values).
			None
		}
	}

	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			Aura::authorities().into_inner()
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}

		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}

		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}

		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{baseline, Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use frame_system_benchmarking::Pallet as SystemBench;
			use baseline::Pallet as BaselineBench;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			(list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{baseline, Benchmarking, BenchmarkBatch};
			use frame_support::traits::TrackedStorageKey;

			use frame_system_benchmarking::Pallet as SystemBench;
			use baseline::Pallet as BaselineBench;

			impl frame_system_benchmarking::Config for Runtime {}
			impl baseline::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];
			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);
			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}

	impl module_issue_rpc_runtime_api::IssueApi<
		Block,
		AccountId,
		H256,
		IssueRequest<AccountId, BlockNumber, Balance, CurrencyId>
	> for Runtime {
		fn get_issue_requests(account_id: AccountId) -> Vec<H256> {
			Issue::get_issue_requests_for_account(account_id)
		}

		fn get_vault_issue_requests(vault_id: AccountId) -> Vec<H256> {
			Issue::get_issue_requests_for_vault(vault_id)
		}
	}

	impl module_vault_registry_rpc_runtime_api::VaultRegistryApi<
		Block,
		VaultId,
		Balance,
		UnsignedFixedPoint,
		CurrencyId,
		AccountId,
	> for Runtime {
		fn get_vault_collateral(vault_id: VaultId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let result = VaultRegistry::compute_collateral(&vault_id)?;
			Ok(BalanceWrapper{amount:result.amount()})
		}

		fn get_vaults_by_account_id(account_id: AccountId) -> Result<Vec<VaultId>, DispatchError> {
			VaultRegistry::get_vaults_by_account_id(account_id)
		}

		fn get_vault_total_collateral(vault_id: VaultId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let result = VaultRegistry::get_backing_collateral(&vault_id)?;
			Ok(BalanceWrapper{amount:result.amount()})
		}

		fn get_premium_redeem_vaults() -> Result<Vec<(VaultId, BalanceWrapper<Balance>)>, DispatchError> {
			let result = VaultRegistry::get_premium_redeem_vaults()?;
			Ok(result.iter().map(|v| (v.0.clone(), BalanceWrapper{amount:v.1.amount()})).collect())
		}

		fn get_vaults_with_issuable_tokens() -> Result<Vec<(VaultId, BalanceWrapper<Balance>)>, DispatchError> {
			let result = VaultRegistry::get_vaults_with_issuable_tokens()?;
			Ok(result.into_iter().map(|v| (v.0, BalanceWrapper{amount:v.1.amount()})).collect())
		}

		fn get_vaults_with_redeemable_tokens() -> Result<Vec<(VaultId, BalanceWrapper<Balance>)>, DispatchError> {
			let result = VaultRegistry::get_vaults_with_redeemable_tokens()?;
			Ok(result.into_iter().map(|v| (v.0, BalanceWrapper{amount:v.1.amount()})).collect())
		}

		fn get_issuable_tokens_from_vault(vault: VaultId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let result = VaultRegistry::get_issuable_tokens_from_vault(&vault)?;
			Ok(BalanceWrapper{amount:result.amount()})
		}

		fn get_collateralization_from_vault(vault: VaultId, only_issued: bool) -> Result<UnsignedFixedPoint, DispatchError> {
			VaultRegistry::get_collateralization_from_vault(vault, only_issued)
		}

		fn get_collateralization_from_vault_and_collateral(vault: VaultId, collateral: BalanceWrapper<Balance>, only_issued: bool) -> Result<UnsignedFixedPoint, DispatchError> {
			let amount = Amount::new(collateral.amount, vault.collateral_currency());
			VaultRegistry::get_collateralization_from_vault_and_collateral(vault, &amount, only_issued)
		}

		fn get_required_collateral_for_wrapped(amount_wrapped: BalanceWrapper<Balance>, wrapped_currency_id: CurrencyId,  collateral_currency_id: CurrencyId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let amount_wrapped = Amount::new(amount_wrapped.amount, wrapped_currency_id);
			let result = VaultRegistry::get_required_collateral_for_wrapped(&amount_wrapped, collateral_currency_id)?;
			Ok(BalanceWrapper{amount:result.amount()})
		}

		fn get_required_collateral_for_vault(vault_id: VaultId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let result = VaultRegistry::get_required_collateral_for_vault(vault_id)?;
			Ok(BalanceWrapper{amount:result.amount()})
		}
	}

	impl module_redeem_rpc_runtime_api::RedeemApi<
		Block,
		AccountId,
		H256,
		RedeemRequest<AccountId, BlockNumber, Balance, CurrencyId>
	> for Runtime {
		fn get_redeem_requests(account_id: AccountId) -> Vec<H256> {
			Redeem::get_redeem_requests_for_account(account_id)
		}

		fn get_vault_redeem_requests(vault_account_id: AccountId) -> Vec<H256> {
			Redeem::get_redeem_requests_for_vault(vault_account_id)
		}
	}

	impl module_replace_rpc_runtime_api::ReplaceApi<
		Block,
		AccountId,
		H256,
		ReplaceRequest<AccountId, BlockNumber, Balance, CurrencyId>
	> for Runtime {
		fn get_old_vault_replace_requests(vault_id: AccountId) -> Vec<H256> {
			Replace::get_replace_requests_for_old_vault(vault_id)
		}

		fn get_new_vault_replace_requests(vault_id: AccountId) -> Vec<H256> {
			Replace::get_replace_requests_for_new_vault(vault_id)
		}
	}

	impl module_oracle_rpc_runtime_api::OracleApi<
		Block,
		Balance,
		CurrencyId
	> for Runtime {
		fn currency_to_usd(amount: BalanceWrapper<Balance>, currency_id: CurrencyId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let result = Oracle::currency_to_usd(amount.amount, currency_id)?;
			Ok(BalanceWrapper{amount:result})
		}

		fn usd_to_currency(amount: BalanceWrapper<Balance>, currency_id: CurrencyId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let result = Oracle::usd_to_currency(amount.amount, currency_id)?;
			Ok(BalanceWrapper{amount:result})
		}

		fn get_exchange_rate(currency_id: CurrencyId) -> Result<UnsignedFixedPoint, DispatchError> {
			let result = Oracle::get_exchange_rate(currency_id)?;
			Ok(result)
		}
	}

	impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
		fn create_default_config() -> Vec<u8> {
			create_default_config::<RuntimeGenesisConfig>()
		}

		fn build_config(config: Vec<u8>) -> sp_genesis_builder::Result {
			build_config::<RuntimeGenesisConfig>(config)
		}
	}

}
