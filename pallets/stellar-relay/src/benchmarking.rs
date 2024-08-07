//! Benchmarking setup for pallet-template

use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::BoundedVec;
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use sp_std::vec;

#[allow(unused)]
use crate::Pallet as StellarRelay;
use crate::{
	traits::{FieldLength, Organization, Validator},
	types::{OrganizationOf, ValidatorOf},
};

use super::*;

benchmarks! {
	update_tier_1_validator_set {
		let caller: T::AccountId = whitelisted_caller();

		let bounded_vec = BoundedVec::<u8, FieldLength>::default();

		let validator: ValidatorOf<T> = Validator {
			name: bounded_vec.clone(),
			public_key: bounded_vec.clone(),
			organization_id: 0.into(),
		};

		let mut validators = vec![validator; 255];

		for (i, validator) in validators.iter_mut().enumerate() {
			validator.public_key = BoundedVec::<u8, FieldLength>::try_from(vec![i as u8; 128]).unwrap();
		}

		let organization: OrganizationOf<T> = Organization {
			id: T::OrganizationId::default(),
			name: bounded_vec,
		};

		let organizations = vec![organization; 1];
		let enactment_block_height = BlockNumberFor::<T>::default();

		// After calling the extrinsic, the current validators and organizations will be the old ones
		let old_organizations = Organizations::<T>::get();
		let old_validators = Validators::<T>::get();
	}: update_tier_1_validator_set(RawOrigin::Root, validators.clone(), organizations.clone(), enactment_block_height)
	verify {
		assert_eq!(OldOrganizations::<T>::get(), old_organizations);
		assert_eq!(OldValidators::<T>::get(), old_validators);
		assert_eq!(Organizations::<T>::get(), BoundedVec::<OrganizationOf<T>, T::OrganizationLimit>::try_from(organizations).unwrap());
		assert_eq!(Validators::<T>::get(), BoundedVec::<ValidatorOf<T>, T::ValidatorLimit>::try_from(validators).unwrap());
	}

}

impl_benchmark_test_suite!(
	StellarRelay,
	crate::mock::ExtBuilder::build(Default::default(), Default::default()),
	crate::mock::Test
);
