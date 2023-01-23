use std::{cell::RefCell, sync::RwLock};

use dia_oracle::{CoinInfo, DiaOracle};
use frame_support::{
	parameter_types,
	traits::{ConstU32, Everything, GenesisBuild},
};
use mocktopus::mocking::clear_mocks;
use orml_oracle::{DataProvider, TimestampedValue};
use orml_traits::parameter_type_with_key;
use primitives::oracle::Key;
use sp_arithmetic::{FixedI128, FixedU128};
use sp_core::{sr25519::Signature, H256};
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{
		BlakeTwo256, Convert, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify,
	},
};

pub use currency::testing_constants::{
	DEFAULT_COLLATERAL_CURRENCY, DEFAULT_NATIVE_CURRENCY, DEFAULT_WRAPPED_CURRENCY,
};
pub use primitives::ForeignCurrencyId::*;

use crate::{
	self as oracle,
	dia::DiaOracleAdapter,
	oracle_mock::{Data, MockConvertMoment, MockConvertPrice, MockOracleKeyConvertor},
	Config, Error, OracleKeys,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		// substrate pallets
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Tokens: orml_tokens::{Pallet, Storage, Config<T>, Event<T>},

		// Operational
		Security: security::{Pallet, Call, Storage, Event<T>},
		Oracle: oracle::{Pallet, Call, Config, Storage, Event<T>},
		Staking: staking::{Pallet, Storage, Event<T>},
		Currency: currency::{Pallet},
	}
);

pub type AccountId = u64;
pub type Balance = u128;
pub type BlockNumber = u64;
pub type UnsignedFixedPoint = FixedU128;
pub type SignedFixedPoint = FixedI128;
pub type SignedInner = i128;
pub type CurrencyId = primitives::CurrencyId;
pub type Moment = u64;
pub type Index = u64;

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
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const GetCollateralCurrencyId: CurrencyId = DEFAULT_COLLATERAL_CURRENCY;
	pub const GetNativeCurrencyId: CurrencyId = DEFAULT_NATIVE_CURRENCY;
	pub const MaxLocks: u32 = 50;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		0
	};
}

pub struct CurrencyHooks<T>(sp_std::marker::PhantomData<T>);
impl<T: orml_tokens::Config>
	orml_traits::currency::MutationHooks<T::AccountId, T::CurrencyId, T::Balance> for CurrencyHooks<T>
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

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = primitives::Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = CurrencyHooks<Self>;
	type MaxLocks = MaxLocks;
	type MaxReserves = ConstU32<0>;
	type ReserveIdentifier = ();
	type DustRemovalWhitelist = Everything;
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
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type GetRelayChainCurrencyId = GetCollateralCurrencyId;

	type AssetConversion = primitives::AssetConversion;
	type BalanceConversion = primitives::BalanceConversion;
	type CurrencyConversion = CurrencyConvert;
}

impl Config for Test {
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

parameter_types! {
	pub const MinimumPeriod: Moment = 5;
}

impl pallet_timestamp::Config for Test {
	type Moment = Moment;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
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

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build() -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		frame_support::traits::GenesisBuild::<Test>::assimilate_storage(
			&oracle::GenesisConfig { oracle_keys: vec![], max_delay: 0 },
			&mut storage,
		)
		.unwrap();

		sp_io::TestExternalities::from(storage)
	}
}

pub fn run_test<T>(test: T)
where
	T: FnOnce(),
{
	clear_mocks();
	ExtBuilder::build().execute_with(|| {
		Security::set_active_block_number(1);
		System::set_block_number(1);
		test();
		// COINS.write().unwrap().clear();
	});
}

thread_local! {
	static COINS: RefCell<Vec<Data>> = RefCell::new(vec![]);
}

pub struct MockDiaOracle;
impl DiaOracle for MockDiaOracle {
	fn get_coin_info(
		blockchain: Vec<u8>,
		symbol: Vec<u8>,
	) -> Result<dia_oracle::CoinInfo, sp_runtime::DispatchError> {
		let key = (blockchain, symbol);
		let mut result: Option<Data> = None;
		COINS.with(|c| {
			let r = c.borrow();
			for i in &*r {
				if i.key == key.clone() {
					result = Some(i.clone());
					break
				}
			}
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
	) -> Result<dia_oracle::PriceInfo, sp_runtime::DispatchError> {
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
impl orml_oracle::DataFeeder<Key, TimestampedValue<UnsignedFixedPoint, Moment>, AccountId>
	for DataCollector
{
	fn feed_value(
		who: AccountId,
		key: Key,
		value: TimestampedValue<UnsignedFixedPoint, Moment>,
	) -> sp_runtime::DispatchResult {
		let key = MockOracleKeyConvertor::convert(key).unwrap();
		let r = value.value.into_inner();

		let data = Data { key, price: r, timestamp: value.timestamp };

		COINS.with(|coins| {
			let mut r = coins.borrow_mut();
			r.push(data)
		});
		Ok(())
	}
}
