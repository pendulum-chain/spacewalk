use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError::BadOrigin;
use substrate_stellar_sdk::{types::EnvelopeType, TransactionEnvelope, XdrCodec};

use crate::{mock::*, traits::Validator, Error};

#[test]
fn validate_stellar_transaction_works_for_correct_values() {
	new_test_ext().execute_with(|| {
		let transaction_envelope = TransactionEnvelope::Default(EnvelopeType::EnvelopeTypeTx);
		let transaction_envelope_xdr = transaction_envelope.to_xdr();

		let externalized_messages = vec![];
		let transaction_set = vec![];
		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			transaction_envelope_xdr,
			externalized_messages,
			transaction_set
		));
	});
}

#[test]
fn validate_stellar_transaction_fails_for_bad_values() {
	new_test_ext().execute_with(|| {
		let transaction_envelope = TransactionEnvelope::Default(EnvelopeType::EnvelopeTypeTx);
		let transaction_envelope_xdr = transaction_envelope.to_xdr();

		let externalized_messages = vec![];
		let transaction_set = vec![];
		let result = SpacewalkRelay::validate_stellar_transaction(
			transaction_envelope_xdr,
			externalized_messages,
			transaction_set,
		);
		assert!(result.is_err());
	});
}

#[test]
fn update_tier_1_validator_set_fails_for_non_root_origin() {
	new_test_ext().execute_with(|| {
		// Ensure the expected error is thrown when no value is present.
		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(Origin::signed(1), vec![]),
			BadOrigin
		);
	});
}

#[test]
fn update_tier_1_validator_set_works() {
	new_test_ext().execute_with(|| {
		let validator = Validator {
			name: Default::default(),
			public_key: Default::default(),
			organization: Default::default(),
			total_org_nodes: 0,
		};
		let validator_set = vec![validator; 3];
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(Origin::root(), validator_set));
	});
}

#[test]
fn update_tier_1_validator_set_fails_when_validator_set_too_large() {
	new_test_ext().execute_with(|| {
		let validator = Validator {
			name: Default::default(),
			public_key: Default::default(),
			organization: Default::default(),
			total_org_nodes: 0,
		};
		// 255 is configured as limit in the test runtime so we try 256
		let validator_set = vec![validator; 256];
		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(Origin::root(), validator_set),
			Error::<Test>::ValidatorLimitExceeded
		);
	});
}
