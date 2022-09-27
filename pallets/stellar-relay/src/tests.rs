use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError::BadOrigin;
use substrate_stellar_sdk::{
	compound_types::{LimitedVarArray, LimitedVarOpaque, UnlimitedVarArray, UnlimitedVarOpaque},
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::{
		EnvelopeType, Preconditions, ScpBallot, ScpEnvelope, ScpStatement, ScpStatementExternalize,
		ScpStatementPledges, Signature, TransactionExt, TransactionSet, TransactionV1Envelope,
		Value,
	},
	AccountId, Hash, Memo, MuxedAccount, PublicKey, Transaction, TransactionEnvelope, XdrCodec,
};

use crate::{
	mock::*,
	traits::{Organization, Validator},
	Error,
};

fn create_dummy_externalize_message(
	keypair: &substrate_stellar_sdk::SecretKey,
	network: &Network,
) -> ScpEnvelope {
	let value: Value = UnlimitedVarOpaque::new([0; 32].to_vec()).unwrap();
	let commit = ScpBallot { counter: 1, value };

	let externalize = ScpStatementExternalize { commit, n_h: 1, commit_quorum_set_hash: [0; 32] };

	let pledges = ScpStatementPledges::ScpStExternalize(externalize);

	let node_id = keypair.get_public().clone();
	let statement = ScpStatement { node_id, slot_index: 1u64, pledges };

	let network = network.get_id();
	let envelope_type_scp = [0, 0, 0, 1].to_vec(); // xdr representation
	let body: Vec<u8> = [network.to_vec(), envelope_type_scp, statement.to_xdr()].concat();

	let signature_result = keypair.create_signature(body);
	let signature: Signature = LimitedVarOpaque::new(signature_result.to_vec()).unwrap();

	let envelope = ScpEnvelope { statement: statement.clone(), signature: signature.clone() };

	envelope
}

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

		let network = &TEST_NETWORK;

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

		let network = &TEST_NETWORK;

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
			organization: Organization { name: Default::default(), total_org_nodes: 0 },
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
			organization: Organization { name: Default::default(), total_org_nodes: 0 },
		};
		// 255 is configured as limit in the test runtime so we try 256
		let validator_set = vec![validator; 256];
		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(Origin::root(), validator_set),
			Error::<Test>::ValidatorLimitExceeded
		);
	});
}

#[test]
fn verify_signature_works_for_xdr_message() {
	let network = &PUBLIC_NETWORK;
	let envelope_base64_xdr = "AAAAAAaweClXqq3sjNIHBm/r6o1RY6yR5HqkHJCaZtEEdMUfAAAAAAIpSk4AAAACAAAAAQAAAJhDlpNWjI0kZ2RCow2qCtM0XCBeAzcd81xKMpGnrYm/4AAAAABg5fhQAAAAAAAAAAEAAAAAM839PPSEV+SDXUw2Ky9ZXf/dPIVBSMk1jlWp9l+9CnsAAABAG46KDK74Y05yGtNqWKoogWBYsfc3OcIdJ49F/BV6OvN5ADZiiuPoZF1Dweo2XN3BxazSDe1u/X8TRPznHxRuDAAAAAE0GgquKerSx6XX8BoZ5cl/XTFD+9K+A5vfE3emLyp4aAAAAEBuChnRV0BBbiJe2dwhkMF+hXW6Nrq9ODUBUSHEq0wOvUnNgrVkLpvP0QTBana8Oscw2xXWMVwR/86ae3VuMXAE";
	let envelope = ScpEnvelope::from_base64_xdr(envelope_base64_xdr).unwrap();
	let node_id: &PublicKey = &envelope.statement.node_id;

	let is_valid = crate::verify_signature(&envelope, &node_id, network);

	assert!(is_valid)
}

#[test]
fn verify_signature_works_for_mock_message() {
	let secret = substrate_stellar_sdk::SecretKey::from_binary([0; 32]);
	let network = &TEST_NETWORK;
	let envelope: ScpEnvelope = create_dummy_externalize_message(&secret, network);
	let node_id: &PublicKey = secret.get_public();

	let is_valid = crate::verify_signature(&envelope, &node_id, network);

	assert!(is_valid)
}
