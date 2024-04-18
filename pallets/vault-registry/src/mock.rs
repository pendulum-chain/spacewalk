use frame_support::{
	parameter_types,
	traits::{ConstU32, ConstU64, Everything, GenesisBuild},
	PalletId,
};
use mocktopus::{macros::mockable, mocking::clear_mocks};
use oracle::{
	dia::DiaOracleAdapter,
	testing_utils::{
		MockConvertMoment, MockConvertPrice, MockDataFeeder, MockDiaOracle, MockOracleKeyConvertor,
	},
};

use orml_currencies::BasicCurrencyAdapter;
use orml_traits::parameter_type_with_key;
use sp_arithmetic::{FixedI128, FixedPointNumber, FixedU128};
use sp_core::H256;
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, IdentityLookup, One, Zero},
	DispatchError, Perquintill,
};

pub use currency::testing_constants::{
	DEFAULT_COLLATERAL_CURRENCY, DEFAULT_NATIVE_CURRENCY, DEFAULT_WRAPPED_CURRENCY,
};
pub use primitives::{CurrencyId, CurrencyId::XCM};
use primitives::{DefaultDecimalsLookup, VaultCurrencyPair, VaultId};

use crate as vault_registry;
use crate::{Config, Error};

pub(crate) type Extrinsic = TestXt<RuntimeCall, ()>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},

		// Tokens & Balances
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Config<T>, Event<T>},
		Currencies: orml_currencies::{Pallet, Call},

		Rewards: pooled_rewards::{Pallet, Call, Storage, Event<T>},
		RewardDistribution: reward_distribution::{Pallet, Storage, Event<T>},

		// Operational
		Security: security::{Pallet, Call, Storage, Event<T>},
		VaultRegistry: vault_registry::{Pallet, Call, Config<T>, Storage, Event<T>, ValidateUnsigned},
		Oracle: oracle::{Pallet, Call, Config, Storage, Event<T>},
		Staking: staking::{Pallet, Storage, Event<T>},
		Fee: fee::{Pallet, Call, Config<T>, Storage},
		Currency: currency::{Pallet},

	}
);

pub type AccountId = u64;
pub type Balance = u128;
pub type RawAmount = i128;
pub type BlockNumber = u64;
pub type Moment = u64;
pub type Index = u64;
pub type SignedFixedPoint = FixedI128;
pub type SignedInner = i128;
pub type UnsignedFixedPoint = FixedU128;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = TestEvent;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 1000;
	pub const MaxReserves: u32 = 50;
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
	type HoldIdentifier = RuntimeHoldReason;
}

impl orml_currencies::Config for Test {
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, i128, BlockNumber>;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}

pub const DEFAULT_CURRENCY_PAIR: VaultCurrencyPair<CurrencyId> = VaultCurrencyPair {
	collateral: DEFAULT_COLLATERAL_CURRENCY,
	wrapped: DEFAULT_WRAPPED_CURRENCY,
};

pub const OTHER_CURRENCY_PAIR: VaultCurrencyPair<CurrencyId> =
	VaultCurrencyPair { collateral: XCM(1), wrapped: DEFAULT_WRAPPED_CURRENCY };

parameter_types! {
	pub const GetCollateralCurrencyId: CurrencyId = DEFAULT_COLLATERAL_CURRENCY;
	pub const GetNativeCurrencyId: CurrencyId = DEFAULT_NATIVE_CURRENCY;
	pub const MaxLocks: u32 = 50;
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
	// we don't use named reserves
	type ReserveIdentifier = ();
	type DustRemovalWhitelist = Everything;
}

parameter_types! {
	pub const MinimumPeriod: Moment = 5;
}

impl pallet_timestamp::Config for Test {
	type Moment = Moment;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

impl oracle::Config for Test {
	type RuntimeEvent = TestEvent;
	type WeightInfo = oracle::SubstrateWeight<Test>;
	type DecimalsLookup = DefaultDecimalsLookup;
	type DataProvider = DiaOracleAdapter<
		MockDiaOracle,
		UnsignedFixedPoint,
		Moment,
		MockOracleKeyConvertor,
		MockConvertPrice,
		MockConvertMoment<Moment>,
	>;
	type DataFeeder = MockDataFeeder<AccountId, Moment>;
}

pub struct CurrencyConvert;
impl currency::CurrencyConversion<currency::Amount<Test>, CurrencyId> for CurrencyConvert {
	fn convert(
		amount: &currency::Amount<Test>,
		to: CurrencyId,
	) -> Result<currency::Amount<Test>, sp_runtime::DispatchError> {
		convert_to(to, *amount)
	}
}

#[cfg_attr(test, mockable)]
pub fn convert_to(
	to: CurrencyId,
	amount: currency::Amount<Test>,
) -> Result<currency::Amount<Test>, sp_runtime::DispatchError> {
	<oracle::Pallet<Test>>::convert(&amount, to)
}

impl currency::Config for Test {
	type UnsignedFixedPoint = UnsignedFixedPoint;
	type SignedInner = SignedInner;
	type SignedFixedPoint = SignedFixedPoint;
	type Balance = Balance;
	type GetRelayChainCurrencyId = GetCollateralCurrencyId;
	type AssetConversion = primitives::AssetConversion;
	type BalanceConversion = primitives::BalanceConversion;
	type CurrencyConversion = CurrencyConvert;
	type AmountCompatibility = primitives::StellarCompatibility;
}

parameter_types! {
	pub const FeePalletId: PalletId = PalletId(*b"mod/fees");
	pub const MaxExpectedValue: UnsignedFixedPoint = UnsignedFixedPoint::from_inner(<UnsignedFixedPoint as FixedPointNumber>::DIV);
}

impl fee::Config for Test {
	type FeePalletId = FeePalletId;
	type WeightInfo = fee::SubstrateWeight<Test>;
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type UnsignedFixedPoint = UnsignedFixedPoint;
	type UnsignedInner = Balance;
	type VaultRewards = Rewards;
	type VaultStaking = Staking;
	type OnSweep = ();
	type MaxExpectedValue = MaxExpectedValue;
	type RewardDistribution = RewardDistribution;
}

parameter_types! {
	pub const DecayRate: Perquintill = Perquintill::from_percent(5);
	pub const MaxCurrencies: u32 = 10;
}

pub struct OracleApiMock {}
impl oracle::OracleApi<Balance, CurrencyId> for OracleApiMock {
	fn currency_to_usd(
		amount: &Balance,
		currency_id: &CurrencyId,
	) -> Result<Balance, DispatchError> {
		let _native_currency = GetNativeCurrencyId::get();
		let amount_in_usd = match currency_id {
			&XCM(0) => amount * 5,
			&XCM(1) => amount * 10,
			&XCM(2) => amount * 15,
			&XCM(3) => amount * 20,
			&XCM(4) => amount * 35,
			_native_currency => amount * 3,
		};
		Ok(amount_in_usd)
	}
}

impl reward_distribution::Config for Test {
	type RuntimeEvent = TestEvent;
	type WeightInfo = reward_distribution::SubstrateWeight<Test>;
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

parameter_types! {
	pub const MaxRewardCurrencies: u32= 10;
}

impl pooled_rewards::Config for Test {
	type RuntimeEvent = TestEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type PoolId = CurrencyId;
	type PoolRewardsCurrencyId = CurrencyId;
	type StakeId = VaultId<AccountId, CurrencyId>;
	type MaxRewardCurrencies = MaxRewardCurrencies;
}

parameter_types! {
	pub const VaultPalletId: PalletId = PalletId(*b"mod/vreg");
}

impl Config for Test {
	type PalletId = VaultPalletId;
	type RuntimeEvent = TestEvent;
	type Balance = Balance;
	type WeightInfo = vault_registry::SubstrateWeight<Test>;
	type GetGriefingCollateralCurrencyId = GetNativeCurrencyId;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

impl security::Config for Test {
	type RuntimeEvent = TestEvent;
	type WeightInfo = ();
}

impl staking::Config for Test {
	type RuntimeEvent = TestEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type CurrencyId = CurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type MaxRewardCurrencies = MaxRewardCurrencies;
}

pub type TestEvent = RuntimeEvent;
pub type TestError = Error<Test>;
pub type TokensError = orml_tokens::Error<Test>;

pub struct ExtBuilder;

pub const COLLATERAL_1_VAULT_1: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 3,
	currencies: VaultCurrencyPair {
		collateral: DEFAULT_COLLATERAL_CURRENCY,
		wrapped: DEFAULT_WRAPPED_CURRENCY,
	},
};
pub const COLLATERAL_1_VAULT_2: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 4,
	currencies: VaultCurrencyPair {
		collateral: DEFAULT_COLLATERAL_CURRENCY,
		wrapped: DEFAULT_WRAPPED_CURRENCY,
	},
};
pub const RICH_ID: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 5,
	currencies: VaultCurrencyPair {
		collateral: DEFAULT_COLLATERAL_CURRENCY,
		wrapped: DEFAULT_WRAPPED_CURRENCY,
	},
};
pub const COLLATERAL_2_VAULT_1: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 6,
	currencies: VaultCurrencyPair { collateral: XCM(1), wrapped: DEFAULT_WRAPPED_CURRENCY },
};
pub const COLLATERAL_2_VAULT_2: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 7,
	currencies: VaultCurrencyPair { collateral: XCM(1), wrapped: DEFAULT_WRAPPED_CURRENCY },
};
pub const DEFAULT_COLLATERAL: u128 = 100000;
pub const RICH_COLLATERAL: u128 = DEFAULT_COLLATERAL + 100000;
pub const MULTI_VAULT_TEST_IDS: [u64; 4] = [100, 101, 102, 103];
pub const MULTI_VAULT_TEST_COLLATERAL: u128 = 100000;

impl ExtBuilder {
	pub fn build_with(conf: orml_tokens::GenesisConfig<Test>) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		conf.assimilate_storage(&mut storage).unwrap();

		// Parameters to be set in tests
		vault_registry::GenesisConfig::<Test> {
			minimum_collateral_vault: vec![(DEFAULT_COLLATERAL_CURRENCY, 0), (XCM(1), 0)],
			punishment_delay: 0,
			system_collateral_ceiling: vec![
				(DEFAULT_CURRENCY_PAIR, 1_000_000_000_000),
				(OTHER_CURRENCY_PAIR, 1_000_000_000_000),
			],
			secure_collateral_threshold: vec![(DEFAULT_CURRENCY_PAIR, UnsignedFixedPoint::one())],
			premium_redeem_threshold: vec![(DEFAULT_CURRENCY_PAIR, UnsignedFixedPoint::one())],
			liquidation_collateral_threshold: vec![(
				DEFAULT_CURRENCY_PAIR,
				UnsignedFixedPoint::one(),
			)],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		sp_io::TestExternalities::from(storage)
	}
	pub fn build() -> sp_io::TestExternalities {
		ExtBuilder::build_with(orml_tokens::GenesisConfig::<Test> {
			balances: vec![
				(COLLATERAL_1_VAULT_1.account_id, DEFAULT_COLLATERAL_CURRENCY, DEFAULT_COLLATERAL),
				(COLLATERAL_1_VAULT_2.account_id, DEFAULT_COLLATERAL_CURRENCY, DEFAULT_COLLATERAL),
				(RICH_ID.account_id, DEFAULT_COLLATERAL_CURRENCY, RICH_COLLATERAL),
				(COLLATERAL_2_VAULT_1.account_id, XCM(1), DEFAULT_COLLATERAL),
				(COLLATERAL_2_VAULT_2.account_id, XCM(1), DEFAULT_COLLATERAL),
				(MULTI_VAULT_TEST_IDS[0], DEFAULT_COLLATERAL_CURRENCY, MULTI_VAULT_TEST_COLLATERAL),
				(MULTI_VAULT_TEST_IDS[1], DEFAULT_COLLATERAL_CURRENCY, MULTI_VAULT_TEST_COLLATERAL),
				(MULTI_VAULT_TEST_IDS[2], DEFAULT_COLLATERAL_CURRENCY, MULTI_VAULT_TEST_COLLATERAL),
				(MULTI_VAULT_TEST_IDS[3], DEFAULT_COLLATERAL_CURRENCY, MULTI_VAULT_TEST_COLLATERAL),
			],
		})
	}
}

pub(crate) fn set_default_thresholds() {
	let secure = UnsignedFixedPoint::checked_from_rational(200, 100).unwrap(); // 200%
	let premium = UnsignedFixedPoint::checked_from_rational(120, 100).unwrap(); // 120%
	let liquidation = UnsignedFixedPoint::checked_from_rational(110, 100).unwrap(); // 110%

	VaultRegistry::_set_secure_collateral_threshold(DEFAULT_CURRENCY_PAIR, secure);
	VaultRegistry::_set_premium_redeem_threshold(DEFAULT_CURRENCY_PAIR, premium);
	VaultRegistry::_set_liquidation_collateral_threshold(DEFAULT_CURRENCY_PAIR, liquidation);

	VaultRegistry::_set_secure_collateral_threshold(OTHER_CURRENCY_PAIR, secure);
	VaultRegistry::_set_premium_redeem_threshold(OTHER_CURRENCY_PAIR, premium);
	VaultRegistry::_set_liquidation_collateral_threshold(OTHER_CURRENCY_PAIR, liquidation);
}

pub fn run_test<T>(test: T)
where
	T: FnOnce(),
{
	clear_mocks();

	// Acquire lock to prevent other tests from changing the exchange rates during the test
	let oracle_mock_lock = <oracle::Pallet<Test>>::acquire_lock();
	<oracle::Pallet<Test>>::clear_values().expect("Failed to clear values");

	ExtBuilder::build().execute_with(|| {
		System::set_block_number(1);
		Security::set_active_block_number(1);
		set_default_thresholds();
		<oracle::Pallet<Test>>::_set_exchange_rate(
			1,
			DEFAULT_COLLATERAL_CURRENCY,
			UnsignedFixedPoint::one(),
		)
		.unwrap();
		<oracle::Pallet<Test>>::_set_exchange_rate(
			1,
			DEFAULT_WRAPPED_CURRENCY,
			UnsignedFixedPoint::one(),
		)
		.unwrap();
		test();
	});

	drop(oracle_mock_lock);
}
