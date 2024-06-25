use frame_support::{
	parameter_types,
	traits::{ConstU16, ConstU64},
	BoundedVec,
};
use frame_system as system;
use primitives::stellar::SecretKey;
use rand::Rng;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage
};

use crate as pallet_spacewalk_relay;
use crate::{
	traits::{FieldLength, Organization, Validator},
	types::{OrganizationOf, ValidatorOf},
	Error,
};

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test 
	{
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		SpacewalkRelay: pallet_spacewalk_relay::{Pallet, Call, Storage, Event<T>},
	}
);

impl system::Config for Test {
	type Block = Block;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const OrganizationLimit: u32 = 255;
	pub const ValidatorLimit: u32 = 255;
	pub const IsPublicNetwork: bool = true;
}

pub type OrganizationId = u128;

impl pallet_spacewalk_relay::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type OrganizationId = OrganizationId;
	type OrganizationLimit = OrganizationLimit;
	type ValidatorLimit = ValidatorLimit;
	type IsPublicNetwork = IsPublicNetwork;
	type WeightInfo = pallet_spacewalk_relay::SubstrateWeight<Test>;
}

fn create_bounded_vec<T: Clone>(input: &[T]) -> Result<BoundedVec<T, FieldLength>, Error<Test>> {
	let bounded_vec = BoundedVec::try_from(input.to_vec())
		.map_err(|_| Error::<Test>::BoundedVecCreationFailed)?;
	Ok(bounded_vec)
}

pub fn create_dummy_validator(
	name: &str,
	organization: &OrganizationOf<Test>,
) -> (ValidatorOf<Test>, SecretKey) {
	let rand = &mut rand::thread_rng();
	let validator_secret = SecretKey::from_binary(rand.gen());

	let validator = Validator {
		name: create_bounded_vec(name.as_bytes()).unwrap(),
		public_key: create_bounded_vec(validator_secret.get_public().to_encoding().as_slice())
			.unwrap(),
		organization_id: organization.id,
	};

	(validator, validator_secret)
}

fn create_dummy_validators() -> (Vec<OrganizationOf<Test>>, Vec<ValidatorOf<Test>>, Vec<SecretKey>)
{
	let mut organizations: Vec<OrganizationOf<Test>> = vec![];
	let mut validators: Vec<ValidatorOf<Test>> = vec![];
	// These secret keys are required to be in the same order as the validators in this test
	// They are later used to sign the dummy scp messages
	let mut validator_secret_keys: Vec<SecretKey> = vec![];

	let organization_sdf =
		Organization { name: create_bounded_vec("sdf".as_bytes()).unwrap(), id: 0 };
	organizations.push(organization_sdf.clone());

	let (validator, validator_secret) = create_dummy_validator("$sdf1", &organization_sdf);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);
	let (validator, validator_secret) = create_dummy_validator("$sdf2", &organization_sdf);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);
	let (validator, validator_secret) = create_dummy_validator("$sdf3", &organization_sdf);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);

	let organization_keybase =
		Organization { name: create_bounded_vec("keybase".as_bytes()).unwrap(), id: 1 };
	organizations.push(organization_keybase.clone());

	let (validator, validator_secret) = create_dummy_validator("$keybase1", &organization_keybase);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);
	let (validator, validator_secret) = create_dummy_validator("$keybase2", &organization_keybase);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);
	let (validator, validator_secret) = create_dummy_validator("$keybase3", &organization_keybase);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);

	let organization_satoshipay =
		Organization { name: create_bounded_vec("satoshipay".as_bytes()).unwrap(), id: 2 };
	organizations.push(organization_satoshipay.clone());

	let (validator, validator_secret) =
		create_dummy_validator("$satoshipay-de", &organization_satoshipay);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);
	let (validator, validator_secret) =
		create_dummy_validator("$satoshipay-us", &organization_satoshipay);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);
	let (validator, validator_secret) =
		create_dummy_validator("$satoshipay-sg", &organization_satoshipay);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);

	let organization_lobstr =
		Organization { name: create_bounded_vec("lobstr".as_bytes()).unwrap(), id: 3 };
	organizations.push(organization_lobstr.clone());

	let (validator, validator_secret) = create_dummy_validator("$lobstr1", &organization_lobstr);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);
	let (validator, validator_secret) = create_dummy_validator("$lobstr2", &organization_lobstr);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);
	let (validator, validator_secret) = create_dummy_validator("$lobstr3", &organization_lobstr);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);
	let (validator, validator_secret) = create_dummy_validator("$lobstr4", &organization_lobstr);
	validators.push(validator);
	validator_secret_keys.push(validator_secret);

	(organizations, validators, validator_secret_keys)
}

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(
		organizations: Vec<OrganizationOf<Test>>,
		validators: Vec<ValidatorOf<Test>>,
	) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
		

		crate::GenesisConfig::<Test> {
			old_validators: vec![],
			old_organizations: vec![],
			validators,
			organizations,
			enactment_block_height: 0,
			phantom: Default::default(),
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		storage.into()
	}
}

pub fn run_test<T>(test: T)
where
	T: FnOnce(Vec<OrganizationOf<Test>>, Vec<ValidatorOf<Test>>, Vec<SecretKey>),
{
	let (organizations, validators, validator_secret_keys) = create_dummy_validators();
	ExtBuilder::build(organizations.clone(), validators.clone()).execute_with(|| {
		test(organizations, validators, validator_secret_keys);
	});
}
