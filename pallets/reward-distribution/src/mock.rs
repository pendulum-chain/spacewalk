use crate as reward_distribution;
use crate::Config;
use frame_support::{
	parameter_types,
	traits::{ConstU32, ConstU64, Everything},
};
use sp_core::H256;
use sp_runtime::{
	generic::Header as GenericHeader,
	traits::{BlakeTwo256, IdentityLookup},
	DispatchError, Perquintill,
};

use sp_arithmetic::FixedI128;

use primitives::{Balance, CurrencyId, VaultId};
type Header = GenericHeader<BlockNumber, BlakeTwo256>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub use currency::testing_constants::DEFAULT_NATIVE_CURRENCY;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Security: security::{Pallet, Call, Storage, Event<T>},
		RewardDistribution: reward_distribution::{Pallet, Call, Storage, Event<T>},

		Rewards: pooled_rewards::{Pallet, Call, Storage, Event<T>},
	}
);

pub type BlockNumber = u64;
pub type Index = u64;
pub type AccountId = u64;
pub type SignedFixedPoint = FixedI128;

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
	pub const MaxCurrencies: u32 = 10;
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

pub struct OracleApiMock {}
impl oracle::OracleApi<Balance, CurrencyId> for OracleApiMock {
	fn currency_to_usd(
		_amount: &Balance,
		currency_id: &CurrencyId,
	) -> Result<Balance, DispatchError> {
		let _native_currency = GetNativeCurrencyId::get();
		match currency_id {
			_native_currency => return Ok(100),
			//_ => unimplemented!("unimplemented mock conversion for currency"),
		}
	}
}
impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = crate::default_weights::SubstrateWeight<Test>;
	type Currency = CurrencyId;
	type Balance = Balance;
	type DecayInterval = ConstU64<100>;
	type DecayRate = DecayRate;
	type VaultRewards = Rewards;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type MaxCurrencies = MaxCurrencies;
	type OracleApi = OracleApiMock;
}

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build() -> sp_io::TestExternalities {
		let storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

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
