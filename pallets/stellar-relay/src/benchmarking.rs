//! Benchmarking setup for pallet-template

use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use sp_std::vec;

use crate::traits::{FieldLength, Organization, Validator};
#[allow(unused)]
use crate::Pallet as StellarRelay;

use super::*;

benchmarks! {
	update_tier_1_validator_set_benchmark {
		let caller: T::AccountId = whitelisted_caller();

		let bounded_vec = BoundedVec::<u8, FieldLength>::default();

		let validator: Validator = Validator {
			name: bounded_vec.clone(),
			public_key: bounded_vec.clone(),
			organization_id: 0,
		};

		let validators = vec![validator; 255];

		let organization: Organization = Organization {
			id: 0,
			name: bounded_vec.clone(),
		};

		let organizations = vec![organization; 255];

	}: update_tier_1_validator_set(RawOrigin::Root, validators.clone(), organizations.clone())
	verify {
		assert_eq!(Organizations::<T>::get(), BoundedVec::<Organization, T::OrganizationLimit>::try_from(organizations).unwrap());
		assert_eq!(Validators::<T>::get(), BoundedVec::<Validator, T::ValidatorLimit>::try_from(validators).unwrap());
	}

	impl_benchmark_test_suite!(StellarRelay, crate::mock::new_test_ext(), crate::mock::Test);
}
