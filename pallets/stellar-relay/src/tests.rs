use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError::BadOrigin;
use substrate_stellar_sdk::{
	compound_types::{LimitedVarArray, UnlimitedVarArray},
	network::Network,
	types::{
		EnvelopeType, Preconditions, ScpEnvelope, TransactionExt, TransactionSet,
		TransactionV1Envelope,
	},
	AccountId, Hash, Memo, MuxedAccount, PublicKey, Transaction, TransactionEnvelope, XdrCodec,
};

use crate::{mock::*, traits::Validator, Error};

#[test]
fn validate_stellar_transaction_works_for_correct_values() {
	new_test_ext().execute_with(|| {
		let source_account =
			MuxedAccount::from(AccountId::from(PublicKey::PublicKeyTypeEd25519([0; 32])));

		let operations = LimitedVarArray::new(vec![]).unwrap();

		let transaction = Transaction {
			source_account,
			fee: 0,
			seq_num: 0,
			cond: Preconditions::PrecondNone,
			memo: Memo::MemoNone,
			operations,
			ext: TransactionExt::V0,
		};

		let transaction_envelope: TransactionEnvelope =
			TransactionEnvelope::EnvelopeTypeTx(TransactionV1Envelope {
				tx: transaction,
				signatures: LimitedVarArray::new(vec![]).unwrap(),
			});

		let envelopes = UnlimitedVarArray::<ScpEnvelope>::new_empty();

		let mut txes = UnlimitedVarArray::<TransactionEnvelope>::new_empty();
		// Add the transaction that is to be verified to the transaction set
		txes.push(transaction_envelope.clone()).unwrap();
		let transaction_set = TransactionSet { previous_ledger_hash: Hash::default(), txes };

		let network = Network::new(b"Public Global Stellar Network ; September 2015");

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			transaction_envelope,
			envelopes,
			transaction_set,
			network
		));
	});
}

#[test]
fn validate_stellar_transaction_fails_for_bad_values() {
	new_test_ext().execute_with(|| {
		let transaction_envelope = TransactionEnvelope::Default(EnvelopeType::EnvelopeTypeTx);

		let externalized_messages = UnlimitedVarArray::<ScpEnvelope>::new_empty();

		let transaction_set = TransactionSet {
			previous_ledger_hash: Hash::default(),
			txes: UnlimitedVarArray::<TransactionEnvelope>::new_empty(),
		};

		let network = Network::new(b"Public Global Stellar Network ; September 2015");

		let result = SpacewalkRelay::validate_stellar_transaction(
			transaction_envelope,
			externalized_messages,
			transaction_set,
			network,
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
