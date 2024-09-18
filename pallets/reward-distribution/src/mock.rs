use crate as reward_distribution;
use crate::Config;
use frame_support::{
	parameter_types,
	traits::{ConstU32, ConstU64, Everything},
	PalletId,
};
use orml_currencies::BasicCurrencyAdapter;
use primitives::{Balance, CurrencyId, CurrencyId::XCM, VaultId};
use sp_arithmetic::{FixedI128, FixedU128};
use sp_core::H256;
use frame_support::sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, DispatchError, Perquintill,
};
type Block = frame_system::mocking::MockBlock<Test>;
use orml_traits::parameter_type_with_key;
use sp_arithmetic::traits::Zero;

pub use currency::testing_constants::{DEFAULT_COLLATERAL_CURRENCY, DEFAULT_NATIVE_CURRENCY};

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,

		//Tokens and Balances
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Currencies: orml_currencies,
		RewardDistribution: reward_distribution,
		Rewards: pooled_rewards,
		Staking: staking,

		//Operational
		Security: security,

	}
);

pub type BlockNumber = u64;
pub type Nonce = u64;
pub type AccountId = u64;
pub type SignedFixedPoint = FixedI128;
pub type RawAmount = i128;
pub type SignedInner = i128;
pub type UnsignedFixedPoint = FixedU128;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Test {
	type Block = Block;
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = Nonce;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 1000;
	pub const MaxReserves: u32 = 50;
	pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Test {
	type MaxLocks = MaxLocks;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ConstU32<1>;
	type RuntimeHoldReason = RuntimeHoldReason;
}

impl security::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
	pub const DecayRate: Perquintill = Perquintill::from_percent(5);
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = DEFAULT_NATIVE_CURRENCY;
	pub const GetRelayChainCurrencyId: CurrencyId = DEFAULT_COLLATERAL_CURRENCY;
	pub const MaxCurrencies: u32 = 10;
}

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
	pub const GetWrappedCurrencyId: CurrencyId = currency::testing_constants::DEFAULT_WRAPPED_CURRENCY;
}

parameter_types! {
	pub const MaxRewardCurrencies: u32= 10;
}

impl pooled_rewards::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type PoolId = CurrencyId;
	type PoolRewardsCurrencyId = CurrencyId;
	type StakeId = VaultId<AccountId, CurrencyId>;
	type MaxRewardCurrencies = MaxRewardCurrencies;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Zero::zero()
	};
}

pub struct CurrencyHooks<T>(sp_std::marker::PhantomData<T>);
impl<T: orml_tokens::Config>
	orml_traits::currency::MutationHooks<T::AccountId, T::CurrencyId, T::Balance> for CurrencyHooks<T>
{
	type OnDust = ();
	type OnSlash = ();
	type PreDeposit = ();
	type PostDeposit = ();
	type PreTransfer = ();
	type PostTransfer = ();
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = RawAmount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = CurrencyHooks<Self>;
	type MaxLocks = MaxLocks;
	type MaxReserves = ConstU32<0>;
	type ReserveIdentifier = ();
	type DustRemovalWhitelist = Everything;
}

pub type Amount = i128;

impl orml_currencies::Config for Test {
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}

pub struct CurrencyConvert;
impl currency::CurrencyConversion<currency::Amount<Test>, CurrencyId> for CurrencyConvert {
	fn convert(
		_amount: &currency::Amount<Test>,
		_to: CurrencyId,
	) -> Result<currency::Amount<Test>, sp_runtime::DispatchError> {
		unimplemented!()
	}
}

impl currency::Config for Test {
	type UnsignedFixedPoint = UnsignedFixedPoint;
	type SignedInner = SignedInner;
	type SignedFixedPoint = SignedFixedPoint;
	type Balance = Balance;
	type GetRelayChainCurrencyId = GetRelayChainCurrencyId;
	#[cfg(feature = "runtime-benchmarks")]
	type GetWrappedCurrencyId = GetWrappedCurrencyId;
	type AssetConversion = primitives::AssetConversion;
	type BalanceConversion = primitives::BalanceConversion;
	type CurrencyConversion = CurrencyConvert;
	type AmountCompatibility = primitives::StellarCompatibility;
}

pub struct OracleApiMock {}
impl oracle::OracleApi<Balance, CurrencyId> for OracleApiMock {
	fn currency_to_usd(
		amount: &Balance,
		currency_id: &CurrencyId,
	) -> Result<Balance, DispatchError> {
		let amount_in_usd = match currency_id {
			&XCM(0) => amount * 100,
			&XCM(1) => amount * 10,
			&XCM(2) => amount * 15,
			&XCM(3) => amount * 20,
			&XCM(4) => amount * 35,
			_ => amount * 50,
		};
		Ok(amount_in_usd)
	}
}

impl staking::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type CurrencyId = CurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type MaxRewardCurrencies = MaxRewardCurrencies;
}

parameter_types! {
	pub const FeePalletId: PalletId = PalletId(*b"mod/fees");
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = crate::default_weights::SubstrateWeight<Test>;
	type Balance = Balance;
	type DecayInterval = ConstU64<100>;
	type DecayRate = DecayRate;
	type VaultRewards = Rewards;
	type MaxCurrencies = MaxCurrencies;
	type OracleApi = OracleApiMock;
	type Balances = Balances;
	type VaultStaking = Staking;
	type FeePalletId = FeePalletId;
}

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build() -> sp_io::TestExternalities {
		let storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		storage.into()
	}
}

#[allow(dead_code)]
pub fn run_test<T>(test: T)
where
	T: FnOnce(),
{
	ExtBuilder::build().execute_with(|| {
		System::set_block_number(1);
		test();
	});
}
