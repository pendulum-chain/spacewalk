use frame_support::{
	assert_ok, parameter_types,
	traits::{ConstU32, Everything, GenesisBuild},
	PalletId,
};
use mocktopus::{macros::mockable, mocking::clear_mocks};
use oracle::{
	dia::DiaOracleAdapter,
	oracle_mock::{Data, MockConvertMoment, MockConvertPrice, MockOracleKeyConvertor},
	CoinInfo, DataFeeder, DataProvider, DiaOracle, PriceInfo, TimestampedValue,
};
use orml_traits::parameter_type_with_key;
use primitives::oracle::Key;
use sp_arithmetic::{FixedI128, FixedPointNumber, FixedU128};
use sp_core::H256;
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, Convert, IdentityLookup, One, Zero},
};
use std::cell::RefCell;

pub use currency::{
	testing_constants::{
		DEFAULT_COLLATERAL_CURRENCY, DEFAULT_NATIVE_CURRENCY, DEFAULT_WRAPPED_CURRENCY,
	},
	Amount,
};
pub use primitives::{CurrencyId, ForeignCurrencyId::*};
use primitives::{VaultCurrencyPair, VaultId};

use crate as replace;
use crate::{Config, Error};

type TestExtrinsic = TestXt<RuntimeCall, ()>;
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
		Tokens: orml_tokens::{Pallet, Storage, Config<T>, Event<T>},

		Rewards: reward::{Pallet, Call, Storage, Event<T>},

		// Operational
		StellarRelay: stellar_relay::{Pallet, Call, Config<T>, Storage, Event<T>},
		Security: security::{Pallet, Call, Storage, Event<T>},
		VaultRegistry: vault_registry::{Pallet, Call, Config<T>, Storage, Event<T>},
		Oracle: oracle::{Pallet, Call, Config, Storage, Event<T>},
		Replace: replace::{Pallet, Call, Config<T>, Storage, Event<T>},
		Fee: fee::{Pallet, Call, Config<T>, Storage},
		Nomination: nomination::{Pallet, Call, Config, Storage, Event<T>},
		Staking: staking::{Pallet, Storage, Event<T>},
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
pub type UnsignedInner = u128;

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

impl reward::Config for Test {
	type RuntimeEvent = TestEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type RewardId = VaultId<AccountId, CurrencyId>;
	type CurrencyId = CurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}

parameter_types! {
	pub const VaultPalletId: PalletId = PalletId(*b"mod/vreg");
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = TestExtrinsic;
}
pub struct CurrencyConvert;
impl currency::CurrencyConversion<currency::Amount<Test>, CurrencyId> for CurrencyConvert {
	fn convert(
		amount: &currency::Amount<Test>,
		to: CurrencyId,
	) -> Result<currency::Amount<Test>, sp_runtime::DispatchError> {
		let amount = convert_to(to, amount.amount())?;
		Ok(Amount::new(amount, to))
	}
}

#[cfg_attr(test, mockable)]
pub fn convert_to(to: CurrencyId, amount: Balance) -> Result<Balance, sp_runtime::DispatchError> {
	Ok(amount) // default conversion 1:1 - overwritable with mocktopus
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

impl vault_registry::Config for Test {
	type PalletId = VaultPalletId;
	type RuntimeEvent = TestEvent;
	type Balance = Balance;
	type WeightInfo = ();
	type GetGriefingCollateralCurrencyId = GetNativeCurrencyId;
}

impl staking::Config for Test {
	type RuntimeEvent = TestEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type CurrencyId = CurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}

parameter_types! {
	pub const OrganizationLimit: u32 = 255;
	pub const ValidatorLimit: u32 = 255;
}

pub type OrganizationId = u128;

impl stellar_relay::Config for Test {
	type RuntimeEvent = TestEvent;
	type OrganizationId = OrganizationId;
	type OrganizationLimit = OrganizationLimit;
	type ValidatorLimit = ValidatorLimit;
	type WeightInfo = ();
}

impl security::Config for Test {
	type RuntimeEvent = TestEvent;
	type WeightInfo = ();
}

impl nomination::Config for Test {
	type RuntimeEvent = TestEvent;
	type WeightInfo = ();
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
	type UnsignedInner = UnsignedInner;
	type VaultRewards = Rewards;
	type VaultStaking = Staking;
	type OnSweep = ();
	type MaxExpectedValue = MaxExpectedValue;
}

impl Config for Test {
	type RuntimeEvent = TestEvent;
	type WeightInfo = ();
}

pub type TestEvent = RuntimeEvent;
pub type TestError = Error<Test>;

pub const OLD_VAULT: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 1,
	currencies: VaultCurrencyPair {
		collateral: DEFAULT_COLLATERAL_CURRENCY,
		wrapped: DEFAULT_WRAPPED_CURRENCY,
	},
};
pub const NEW_VAULT: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 2,
	currencies: VaultCurrencyPair {
		collateral: DEFAULT_COLLATERAL_CURRENCY,
		wrapped: DEFAULT_WRAPPED_CURRENCY,
	},
};

pub const OLD_VAULT_BALANCE: u128 = 1_000_000;
pub const NEW_VAULT_BALANCE: u128 = 1_000_000;

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build_with(balances: orml_tokens::GenesisConfig<Test>) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		balances.assimilate_storage(&mut storage).unwrap();

		fee::GenesisConfig::<Test> {
			issue_fee: UnsignedFixedPoint::checked_from_rational(5, 1000).unwrap(), // 0.5%
			issue_griefing_collateral: UnsignedFixedPoint::checked_from_rational(5, 100000)
				.unwrap(), // 0.005%
			redeem_fee: UnsignedFixedPoint::checked_from_rational(5, 1000).unwrap(), // 0.5%
			premium_redeem_fee: UnsignedFixedPoint::checked_from_rational(5, 100).unwrap(), // 5%
			punishment_fee: UnsignedFixedPoint::checked_from_rational(1, 10).unwrap(), // 10%
			replace_griefing_collateral: UnsignedFixedPoint::checked_from_rational(1, 10).unwrap(), // 10%
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		const PAIR: VaultCurrencyPair<CurrencyId> = VaultCurrencyPair {
			collateral: DEFAULT_COLLATERAL_CURRENCY,
			wrapped: DEFAULT_WRAPPED_CURRENCY,
		};
		vault_registry::GenesisConfig::<Test> {
			minimum_collateral_vault: vec![(DEFAULT_COLLATERAL_CURRENCY, 0)],
			punishment_delay: 8,
			system_collateral_ceiling: vec![(PAIR, 1_000_000_000_000)],
			secure_collateral_threshold: vec![(
				PAIR,
				UnsignedFixedPoint::checked_from_rational(200, 100).unwrap(),
			)],
			premium_redeem_threshold: vec![(
				PAIR,
				UnsignedFixedPoint::checked_from_rational(120, 100).unwrap(),
			)],
			liquidation_collateral_threshold: vec![(
				PAIR,
				UnsignedFixedPoint::checked_from_rational(110, 100).unwrap(),
			)],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		replace::GenesisConfig::<Test> { replace_period: 10, replace_minimum_transfer_amount: 2 }
			.assimilate_storage(&mut storage)
			.unwrap();

		storage.into()
	}

	pub fn build() -> sp_io::TestExternalities {
		ExtBuilder::build_with(orml_tokens::GenesisConfig::<Test> {
			balances: vec![
				(OLD_VAULT.account_id, DEFAULT_COLLATERAL_CURRENCY, OLD_VAULT_BALANCE),
				(NEW_VAULT.account_id, DEFAULT_COLLATERAL_CURRENCY, NEW_VAULT_BALANCE),
			],
		})
	}
}

pub fn run_test<T>(test: T)
where
	T: FnOnce(),
{
	clear_mocks();
	ExtBuilder::build().execute_with(|| {
		assert_ok!(<oracle::Pallet<Test>>::_set_exchange_rate(
			1,
			DEFAULT_COLLATERAL_CURRENCY,
			UnsignedFixedPoint::one()
		));
		System::set_block_number(1);
		Security::set_active_block_number(1);
		test();
	});
}

thread_local! {
	static COINS: std::cell::RefCell<Vec<Data>>  = RefCell::new(vec![]);
}

pub struct MockDiaOracle;
impl DiaOracle for MockDiaOracle {
	fn get_coin_info(
		blockchain: Vec<u8>,
		symbol: Vec<u8>,
	) -> Result<CoinInfo, sp_runtime::DispatchError> {
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

	//Spacewalk DiaOracleAdapter does not use get_value function. There is no need to implement this function.
	fn get_value(
		_blockchain: Vec<u8>,
		_symbol: Vec<u8>,
	) -> Result<PriceInfo, sp_runtime::DispatchError> {
		unimplemented!("DiaOracleAdapter implementation of DataProviderExtended does not use this function.")
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

		let data = Data { key, price: r, timestamp: value.timestamp };

		COINS.with(|coins| {
			let mut r = coins.borrow_mut();
			r.push(data)
		});
		Ok(())
	}
}
