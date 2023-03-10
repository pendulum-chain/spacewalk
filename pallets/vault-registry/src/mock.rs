use frame_support::{
	parameter_types,
	traits::{ConstU32, Everything, GenesisBuild},
	PalletId,
};
use mocktopus::{macros::mockable, mocking::clear_mocks};
use oracle::{
	dia::DiaOracleAdapter,
	oracle_mock::{Data, DataKey, MockConvertMoment, MockConvertPrice, MockOracleKeyConvertor},
	CoinInfo, DataFeeder, DataProvider, DiaOracle, PriceInfo, TimestampedValue,
};
use orml_currencies::BasicCurrencyAdapter;
use orml_traits::parameter_type_with_key;
use primitives::oracle::Key;
use sp_arithmetic::{FixedI128, FixedPointNumber, FixedU128};
use sp_core::H256;
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, Convert, IdentityLookup, One, Zero},
};
use std::cell::RefCell;

pub use currency::testing_constants::{
	DEFAULT_COLLATERAL_CURRENCY, DEFAULT_NATIVE_CURRENCY, DEFAULT_WRAPPED_CURRENCY,
};
pub use primitives::CurrencyId;
use primitives::{VaultCurrencyPair, VaultId};

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

		Rewards: reward::{Pallet, Call, Storage, Event<T>},

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

parameter_types! {
	pub const GetCollateralCurrencyId: CurrencyId = DEFAULT_COLLATERAL_CURRENCY;
	pub const GetNativeCurrencyId: CurrencyId = DEFAULT_NATIVE_CURRENCY;
	pub const GetWrappedCurrencyId: CurrencyId = DEFAULT_WRAPPED_CURRENCY;
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

impl reward::Config for Test {
	type RuntimeEvent = TestEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type RewardId = VaultId<AccountId, CurrencyId>;
	type CurrencyId = CurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
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
	type WeightInfo = ();
	type DataProvider = DiaOracleAdapter<
		MockDiaOracle,
		UnsignedFixedPoint,
		Moment,
		MockOracleKeyConvertor,
		MockConvertPrice,
		MockConvertMoment,
	>;
	type DataFeedProvider = DataCollector;
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
}

parameter_types! {
	pub const FeePalletId: PalletId = PalletId(*b"mod/fees");
	pub const MaxExpectedValue: UnsignedFixedPoint = UnsignedFixedPoint::from_inner(<UnsignedFixedPoint as FixedPointNumber>::DIV);
}

impl fee::Config for Test {
	type FeePalletId = FeePalletId;
	type WeightInfo = ();
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type UnsignedFixedPoint = UnsignedFixedPoint;
	type UnsignedInner = Balance;
	type VaultRewards = Rewards;
	type VaultStaking = Staking;
	type OnSweep = ();
	type MaxExpectedValue = MaxExpectedValue;
}

parameter_types! {
	pub const VaultPalletId: PalletId = PalletId(*b"mod/vreg");
}

impl Config for Test {
	type PalletId = VaultPalletId;
	type RuntimeEvent = TestEvent;
	type Balance = Balance;
	type WeightInfo = ();
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
}

pub type TestEvent = RuntimeEvent;
pub type TestError = Error<Test>;
pub type TokensError = orml_tokens::Error<Test>;

pub struct ExtBuilder;

pub const DEFAULT_ID: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 3,
	currencies: VaultCurrencyPair {
		collateral: DEFAULT_COLLATERAL_CURRENCY,
		wrapped: DEFAULT_WRAPPED_CURRENCY,
	},
};
pub const OTHER_ID: VaultId<AccountId, CurrencyId> = VaultId {
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
			minimum_collateral_vault: vec![(DEFAULT_COLLATERAL_CURRENCY, 0)],
			punishment_delay: 0,
			system_collateral_ceiling: vec![(DEFAULT_CURRENCY_PAIR, 1_000_000_000_000)],
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
				(DEFAULT_ID.account_id, DEFAULT_COLLATERAL_CURRENCY, DEFAULT_COLLATERAL),
				(OTHER_ID.account_id, DEFAULT_COLLATERAL_CURRENCY, DEFAULT_COLLATERAL),
				(RICH_ID.account_id, DEFAULT_COLLATERAL_CURRENCY, RICH_COLLATERAL),
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
}

pub fn run_test<T>(test: T)
where
	T: FnOnce(),
{
	clear_mocks();
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
		test()
	})
}

thread_local! {
	static COINS: RefCell<std::collections::HashMap<DataKey, Data>> = RefCell::new(std::collections::HashMap::<DataKey, Data>::new());
}

pub struct MockDiaOracle;
impl DiaOracle for MockDiaOracle {
	fn get_coin_info(
		blockchain: Vec<u8>,
		symbol: Vec<u8>,
	) -> Result<CoinInfo, sp_runtime::DispatchError> {
		let key = (blockchain, symbol);
		let data_key = DataKey { blockchain: key.0.clone(), symbol: key.1.clone() };
		let mut result: Option<Data> = None;
		COINS.with(|c| {
			let r = c.borrow();

			let hash_set = &*r;
			let o = hash_set.get(&data_key);
			match o {
				Some(i) => result = Some(i.clone()),
				None => {},
			};
		});
		let Some(result) = result else {
			return Err(sp_runtime::DispatchError::Other(""));
		};
		let mut coin_info = CoinInfo::default();
		coin_info.price = result.price;
		coin_info.last_update_timestamp = result.timestamp;

		Ok(coin_info)
	}

	//Spacewalk DiaOracleAdapter does not use get_value function. There is no need to implement
	// this function.
	fn get_value(
		_blockchain: Vec<u8>,
		_symbol: Vec<u8>,
	) -> Result<PriceInfo, sp_runtime::DispatchError> {
		unimplemented!(
			"DiaOracleAdapter implementation of DataProviderExtended does not use this function."
		)
	}
}

pub struct DataCollector;
//DataFeeder required to implement DataProvider trait but there no need to implement get function
impl DataProvider<Key, TimestampedValue<UnsignedFixedPoint, Moment>> for DataCollector {
	fn get(_key: &Key) -> Option<TimestampedValue<UnsignedFixedPoint, Moment>> {
		unimplemented!("Not required to implement DataProvider get function")
	}
}
impl DataFeeder<Key, TimestampedValue<UnsignedFixedPoint, Moment>, AccountId> for DataCollector {
	fn feed_value(
		_who: AccountId,
		key: Key,
		value: TimestampedValue<UnsignedFixedPoint, Moment>,
	) -> sp_runtime::DispatchResult {
		let key = MockOracleKeyConvertor::convert(key).unwrap();
		let r = value.value.into_inner();

		let data_key = DataKey { blockchain: key.0.clone(), symbol: key.1.clone() };
		let data = Data { key: data_key.clone(), price: r, timestamp: value.timestamp };

		COINS.with(|coins| {
			let mut r = coins.borrow_mut();
			r.insert(data_key, data);
		});
		Ok(())
	}
}
