use frame_support::{
	assert_ok, parameter_types,
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
use primitives::oracle::Key;
use sp_runtime::traits::Convert;
use std::cell::RefCell;

use orml_traits::parameter_type_with_key;
pub use sp_arithmetic::{FixedI128, FixedPointNumber, FixedU128};
use sp_core::H256;
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, IdentityLookup, One, Zero},
};

pub use currency::{
	testing_constants::{
		DEFAULT_COLLATERAL_CURRENCY, DEFAULT_NATIVE_CURRENCY, DEFAULT_WRAPPED_CURRENCY,
	},
	Amount,
};
pub use oracle::{CurrencyId, OracleKey};
use primitives::{AmountCompatibility, VaultCurrencyPair, VaultId};

use crate as redeem;
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
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Config<T>, Event<T>},
		Currencies: orml_currencies::{Pallet, Call},

		Rewards: pooled_rewards::{Pallet, Call, Storage, Event<T>},

		// Operational
		StellarRelay: stellar_relay::{Pallet, Call, Config<T>, Storage, Event<T>},
		Security: security::{Pallet, Call, Storage, Event<T>},
		VaultRegistry: vault_registry::{Pallet, Call, Config<T>, Storage, Event<T>},
		Oracle: oracle::{Pallet, Call, Config, Storage, Event<T>},
		Redeem: redeem::{Pallet, Call, Config<T>, Storage, Event<T>},
		Fee: fee::{Pallet, Call, Config<T>, Storage},
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

impl vault_registry::Config for Test {
	type PalletId = VaultPalletId;
	type RuntimeEvent = TestEvent;
	type Balance = Balance;
	type WeightInfo = vault_registry::SubstrateWeight<Test>;
	type GetGriefingCollateralCurrencyId = GetNativeCurrencyId;
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

pub struct MockAmountCompatibility;
/// In this mock, we assume that all amounts are compatible with the target currency.
impl AmountCompatibility for MockAmountCompatibility {
	type UnsignedFixedPoint = UnsignedFixedPoint;

	fn is_compatible_with_target(
		_source_amount: <<Self as AmountCompatibility>::UnsignedFixedPoint as FixedPointNumber>::Inner,
	) -> bool {
		true
	}

	fn round_to_compatible_with_target(
		source_amount: <<Self as AmountCompatibility>::UnsignedFixedPoint as FixedPointNumber>::Inner,
	) -> Result<<<Self as AmountCompatibility>::UnsignedFixedPoint as FixedPointNumber>::Inner, ()>
	{
		Ok(source_amount)
	}
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
	type AmountCompatibility = MockAmountCompatibility;
}

impl staking::Config for Test {
	type RuntimeEvent = TestEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type CurrencyId = CurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
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
	pub const OrganizationLimit: u32 = 255;
	pub const ValidatorLimit: u32 = 255;
	pub const IsPublicNetwork: bool = false;
}

pub type OrganizationId = u128;

impl stellar_relay::Config for Test {
	type RuntimeEvent = TestEvent;
	type OrganizationId = OrganizationId;
	type OrganizationLimit = OrganizationLimit;
	type ValidatorLimit = ValidatorLimit;
	type IsPublicNetwork = IsPublicNetwork;
	type WeightInfo = stellar_relay::SubstrateWeight<Test>;
}

impl security::Config for Test {
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
	type WeightInfo = oracle::SubstrateWeight<Test>;
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

const USD: CurrencyId = CurrencyId::XCM(2);

parameter_types! {
	pub const FeePalletId: PalletId = PalletId(*b"mod/fees");
	pub const MaxExpectedValue: UnsignedFixedPoint = UnsignedFixedPoint::from_inner(<UnsignedFixedPoint as FixedPointNumber>::DIV);
	pub const GetBaseCurrency: CurrencyId = USD;
}

impl fee::Config for Test {
	type FeePalletId = FeePalletId;
	type WeightInfo = fee::SubstrateWeight<Test>;
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type UnsignedFixedPoint = UnsignedFixedPoint;
	type UnsignedInner = UnsignedInner;
	type VaultRewards = Rewards;
	type VaultStaking = Staking;
	type BaseCurrency = GetBaseCurrency;
	type OnSweep = ();
	type MaxExpectedValue = MaxExpectedValue;
}

impl Config for Test {
	type RuntimeEvent = TestEvent;
	type WeightInfo = crate::SubstrateWeight<Test>;
}

pub type TestEvent = RuntimeEvent;
pub type TestError = Error<Test>;
pub type VaultRegistryError = vault_registry::Error<Test>;

pub const USER: AccountId = 1;
pub const VAULT: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 2,
	currencies: VaultCurrencyPair {
		collateral: DEFAULT_COLLATERAL_CURRENCY,
		wrapped: DEFAULT_WRAPPED_CURRENCY,
	},
};
pub const CAROL: AccountId = 3;

pub const ALICE_BALANCE: u128 = 1_005_000;
pub const VAULT_BALANCE: u128 = 1_005_000;
pub const CAROL_BALANCE: u128 = 1_005_000;

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build_with(balances: orml_tokens::GenesisConfig<Test>) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: balances
				.balances
				.iter()
				.filter_map(|(account, currency, balance)| match *currency {
					DEFAULT_NATIVE_CURRENCY => Some((*account, *balance)),
					_ => None,
				})
				.collect(),
		}
		.assimilate_storage(&mut storage)
		.unwrap();

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

		vault_registry::GenesisConfig::<Test> {
			minimum_collateral_vault: vec![(DEFAULT_COLLATERAL_CURRENCY, 0)],
			punishment_delay: 8,
			system_collateral_ceiling: vec![(DEFAULT_CURRENCY_PAIR, 1_000_000_000_000)],
			secure_collateral_threshold: vec![(
				DEFAULT_CURRENCY_PAIR,
				UnsignedFixedPoint::checked_from_rational(200, 100).unwrap(),
			)],
			premium_redeem_threshold: vec![(
				DEFAULT_CURRENCY_PAIR,
				UnsignedFixedPoint::checked_from_rational(120, 100).unwrap(),
			)],
			liquidation_collateral_threshold: vec![(
				DEFAULT_CURRENCY_PAIR,
				UnsignedFixedPoint::checked_from_rational(110, 100).unwrap(),
			)],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		redeem::GenesisConfig::<Test> {
			redeem_period: 10,
			redeem_minimum_transfer_amount: 2,
			..redeem::GenesisConfig::<Test>::default()
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		frame_support::traits::GenesisBuild::<Test>::assimilate_storage(
			&oracle::GenesisConfig { oracle_keys: vec![], max_delay: 0 },
			&mut storage,
		)
		.unwrap();

		storage.into()
	}

	pub fn build() -> sp_io::TestExternalities {
		ExtBuilder::build_with(orml_tokens::GenesisConfig::<Test> {
			balances: vec![
				(USER, DEFAULT_COLLATERAL_CURRENCY, ALICE_BALANCE),
				(VAULT.account_id, DEFAULT_COLLATERAL_CURRENCY, VAULT_BALANCE),
				(CAROL, DEFAULT_COLLATERAL_CURRENCY, CAROL_BALANCE),
				(USER, DEFAULT_WRAPPED_CURRENCY, ALICE_BALANCE),
				(VAULT.account_id, DEFAULT_WRAPPED_CURRENCY, VAULT_BALANCE),
				(CAROL, DEFAULT_WRAPPED_CURRENCY, CAROL_BALANCE),
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

		assert_ok!(<oracle::Pallet<Test>>::_set_exchange_rate(
			1,
			DEFAULT_WRAPPED_CURRENCY,
			UnsignedFixedPoint::one()
		));

		<oracle::Pallet<Test>>::begin_block(0);
		Security::set_active_block_number(1);
		System::set_block_number(1);
		test();
	});
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
