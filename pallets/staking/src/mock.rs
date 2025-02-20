use frame_support::{
	parameter_types,
	sp_runtime::traits::{BlakeTwo256, IdentityLookup},
	traits::{ConstU32, Everything},
};
use orml_traits::parameter_type_with_key;
use sp_arithmetic::FixedI128;
use sp_core::H256;

use frame_support::sp_runtime::BuildStorage;

pub use currency::testing_constants::{
	DEFAULT_COLLATERAL_CURRENCY, DEFAULT_NATIVE_CURRENCY, DEFAULT_WRAPPED_CURRENCY,
};
pub use primitives::CurrencyId;
use primitives::{VaultCurrencyPair, VaultId};

use crate as staking;
use crate::{Config, Error};

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Staking: staking,
		Tokens: orml_tokens,
	}
);

pub type AccountId = u64;
pub type Nonce = u64;
pub type SignedFixedPoint = FixedI128;
pub type SignedInner = i128;

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

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = DEFAULT_NATIVE_CURRENCY;
	pub const MaxRewardCurrencies: u32= 10;
}
impl Config for Test {
	type RuntimeEvent = TestEvent;
	type SignedInner = SignedInner;
	type SignedFixedPoint = SignedFixedPoint;
	type CurrencyId = CurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type MaxRewardCurrencies = MaxRewardCurrencies;
}

pub type Balance = u128;
pub type RawAmount = i128;
parameter_types! {
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
	type RuntimeEvent = TestEvent;
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

pub type TestEvent = RuntimeEvent;
pub type TestError = Error<Test>;

pub const VAULT: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 1,
	currencies: VaultCurrencyPair {
		collateral: DEFAULT_COLLATERAL_CURRENCY,
		wrapped: DEFAULT_WRAPPED_CURRENCY,
	},
};
pub const ALICE: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 2,
	currencies: VaultCurrencyPair {
		collateral: DEFAULT_COLLATERAL_CURRENCY,
		wrapped: DEFAULT_WRAPPED_CURRENCY,
	},
};
pub const BOB: VaultId<AccountId, CurrencyId> = VaultId {
	account_id: 3,
	currencies: VaultCurrencyPair {
		collateral: DEFAULT_COLLATERAL_CURRENCY,
		wrapped: DEFAULT_WRAPPED_CURRENCY,
	},
};

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
	ExtBuilder::build().execute_with(|| {
		System::set_block_number(1);
		test();
	});
}
