use crate as security;
use crate::{Config, Error};
pub use frame_support::traits::Everything;
use frame_support::{
	parameter_types,
	sp_runtime::{
		traits::{BlakeTwo256, IdentityLookup},
		BuildStorage,
	},
};
use mocktopus::mocking::clear_mocks;
use sp_core::H256;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Security: security,
	}
);

pub type AccountId = u64;
pub type Nonce = u64;

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
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type RuntimeTask = RuntimeTask;
}

impl Config for Test {
	type RuntimeEvent = TestEvent;
	type WeightInfo = ();
}

pub type TestEvent = RuntimeEvent;
pub type TestError = Error<Test>;

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build() -> sp_io::TestExternalities {
		let storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		storage.into()
	}
}

pub fn run_test<T>(test: T)
where
	T: FnOnce(),
{
	clear_mocks();
	ExtBuilder::build().execute_with(|| {
		System::set_block_number(1);
		Security::set_active_block_number(1);
		test();
	});
}
