pub use currency::{
	testing_constants::{
		DEFAULT_COLLATERAL_CURRENCY, DEFAULT_NATIVE_CURRENCY, DEFAULT_WRAPPED_CURRENCY,
	},
	Amount,
};
use frame_support::{
	assert_ok, parameter_types,
	sp_runtime::{
		testing::TestXt,
		traits::{BlakeTwo256, IdentityLookup, One, Zero},
		BuildStorage, DispatchError, Perquintill,
	},
	traits::{ConstU32, ConstU64, Everything},
	PalletId,
};
use mocktopus::{macros::mockable, mocking::clear_mocks};
pub use oracle::CurrencyId;
use oracle::{
	dia::DiaOracleAdapter,
	testing_utils::{
		MockConvertMoment, MockConvertPrice, MockDataFeeder, MockDiaOracle, MockOracleKeyConvertor,
	},
};
use orml_currencies::BasicCurrencyAdapter;
use orml_traits::parameter_type_with_key;
use primitives::{AmountCompatibility, DefaultDecimalsLookup, VaultCurrencyPair, VaultId};
pub use sp_arithmetic::{FixedI128, FixedPointNumber, FixedU128};
use sp_core::H256;

use crate as redeem;
use crate::{Config, Error};

type TestExtrinsic = TestXt<RuntimeCall, ()>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Timestamp: pallet_timestamp,

		// Tokens & Balances
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Currencies: orml_currencies,

		Rewards: pooled_rewards,
		RewardDistribution: reward_distribution,

		// Operational
		StellarRelay: stellar_relay,
		Security: security,
		VaultRegistry: vault_registry,
		Oracle: oracle,
		Redeem: redeem,
		Fee: fee,
		Staking: staking,
		Currency: currency,
	}
);

//add default to struct Test

pub type AccountId = u64;
pub type Balance = u128;
pub type RawAmount = i128;
pub type BlockNumber = u64;
pub type Moment = u64;
pub type Nonce = u64;
pub type SignedFixedPoint = FixedI128;
pub type SignedInner = i128;
pub type UnsignedFixedPoint = FixedU128;
pub type UnsignedInner = u128;

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
	type RuntimeTask = RuntimeTask;
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
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
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

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
	pub const GetWrappedCurrencyId: CurrencyId = DEFAULT_WRAPPED_CURRENCY;
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
	) -> Result<currency::Amount<Test>, DispatchError> {
		let amount = convert_to(to, amount.amount())?;
		Ok(Amount::new(amount, to))
	}
}

#[cfg_attr(test, mockable)]
pub fn convert_to(to: CurrencyId, amount: Balance) -> Result<Balance, DispatchError> {
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
	#[cfg(feature = "runtime-benchmarks")]
	type GetWrappedCurrencyId = GetWrappedCurrencyId;
	type AssetConversion = primitives::AssetConversion;
	type BalanceConversion = primitives::BalanceConversion;
	type CurrencyConversion = CurrencyConvert;
	type AmountCompatibility = MockAmountCompatibility;
}

parameter_types! {
	pub const MaxRewardCurrencies: u32= 10;
}

impl staking::Config for Test {
	type RuntimeEvent = TestEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type CurrencyId = CurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type MaxRewardCurrencies = MaxRewardCurrencies;
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
	type UnsignedInner = UnsignedInner;
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
		_amount: &Balance,
		currency_id: &CurrencyId,
	) -> Result<Balance, DispatchError> {
		let native_currency = GetNativeCurrencyId::get();
		match currency_id {
			id if *id == native_currency => Ok(100),
			_ => Ok(500),
		}
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
		let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

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

		oracle::GenesisConfig::<Test> {
			oracle_keys: vec![],
			max_delay: 0,
			_phantom: Default::default(),
		}
		.assimilate_storage(&mut storage)
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
