use frame_support::{assert_noop, assert_ok, BoundedVec};
use rand::Rng;
use sp_runtime::DispatchError::BadOrigin;
use substrate_stellar_sdk::{
	compound_types::{LimitedVarArray, LimitedVarOpaque, UnlimitedVarArray, UnlimitedVarOpaque},
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::{
		EnvelopeType, NodeId, Preconditions, ScpBallot, ScpEnvelope, ScpStatement,
		ScpStatementExternalize, ScpStatementPledges, Signature, StellarValue, StellarValueExt,
		TransactionExt, TransactionSet, TransactionV1Envelope, Value,
	},
	AccountId, Hash, Memo, MuxedAccount, PublicKey, SecretKey, Transaction, TransactionEnvelope,
	XdrCodec,
};

use crate::{
	mock::*,
	traits::{FieldLength, Organization, Validator},
	Error,
};

fn create_bounded_vec<T: Clone>(input: &[T]) -> Result<BoundedVec<T, FieldLength>, Error<Test>> {
	let bounded_vec = BoundedVec::try_from(input.to_vec())
		.map_err(|_| Error::<Test>::BoundedVecCreationFailed)?;
	Ok(bounded_vec)
}

fn create_dummy_externalize_message(keypair: &SecretKey, network: &Network) -> ScpEnvelope {
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

fn create_dummy_validator(name: &str, organization: &Organization) -> (Validator, SecretKey) {
	let rand = &mut rand::thread_rng();
	let validator_secret = SecretKey::from_binary(rand.gen());

	let validator = Validator {
		name: create_bounded_vec(name.as_bytes()).unwrap(),
		public_key: create_bounded_vec(validator_secret.get_public().to_encoding().as_slice())
			.unwrap(),
		organization: organization.clone(),
	};

	(validator, validator_secret)
}

fn create_dummy_validators() -> (Vec<Validator>, Vec<SecretKey>) {
	let mut validators: Vec<Validator> = vec![];
	// These secret keys are required to be in the same order as the validators in this test
	// They are later used to sign the scp messages
	let mut validator_secret_keys: Vec<SecretKey> = vec![];

	let organization_satoshipay = Organization {
		name: create_bounded_vec("satoshipay".as_bytes()).unwrap(),
		total_org_nodes: 3,
	};

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
		Organization { name: create_bounded_vec("lobstr".as_bytes()).unwrap(), total_org_nodes: 4 };

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

	(validators, validator_secret_keys)
}

fn create_scp_envelope(
	tx_set_hash: Hash,
	validator: &Validator,
	validator_secret_key: &SecretKey,
	network: &Network,
) -> ScpEnvelope {
	let stellar_value: StellarValue = StellarValue {
		tx_set_hash,
		close_time: 0,
		ext: StellarValueExt::StellarValueBasic,
		upgrades: LimitedVarArray::new_empty(),
	};
	let stellar_value_xdr = stellar_value.to_xdr();
	let value: Value = UnlimitedVarOpaque::new(stellar_value_xdr).unwrap();

	let node_id = NodeId::from_encoding(validator.public_key.clone()).unwrap();

	let statement: ScpStatement = ScpStatement {
		node_id,
		slot_index: 0,
		pledges: ScpStatementPledges::ScpStExternalize(ScpStatementExternalize {
			commit: ScpBallot { counter: 0, value },
			n_h: 0,
			commit_quorum_set_hash: Hash::default(),
		}),
	};

	let network = network.get_id();
	let envelope_type_scp = [0, 0, 0, 1].to_vec(); // xdr representation
	let body: Vec<u8> = [network.to_vec(), envelope_type_scp, statement.to_xdr()].concat();
	let signature_result = validator_secret_key.create_signature(body);
	let signature: Signature = LimitedVarOpaque::new(signature_result.to_vec()).unwrap();

	let envelope = ScpEnvelope { statement, signature };
	envelope
}

fn create_valid_dummy_scp_envelopes(
	validators: Vec<Validator>,
	validator_secret_keys: Vec<SecretKey>,
	network: &Network,
) -> (TransactionEnvelope, TransactionSet, LimitedVarArray<ScpEnvelope, { i32::MAX }>) {
	// Build a transaction
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

	// Build a transaction set with the transaction
	let mut txes = UnlimitedVarArray::<TransactionEnvelope>::new_empty();
	// Add the transaction that is to be verified to the transaction set
	txes.push(transaction_envelope.clone()).unwrap();
	let transaction_set = TransactionSet { previous_ledger_hash: Hash::default(), txes };

	let tx_set_hash = crate::compute_non_generic_tx_set_content_hash(&transaction_set);

	// Build the scp messages that externalize the transaction set
	// The scp messages have to be externalized by nodes that build a valid quorum set

	let mut envelopes = UnlimitedVarArray::<ScpEnvelope>::new_empty();
	for (i, validator_secret_key) in validator_secret_keys.iter().enumerate() {
		let validator = validators.get(i).unwrap();
		let envelope =
			create_scp_envelope(tx_set_hash.clone(), validator, validator_secret_key, network);
		envelopes.push(envelope).unwrap();
	}

	(transaction_envelope, transaction_set, envelopes)
}

#[test]
fn validate_stellar_transaction_fails_for_invalid_quorum() {
	new_test_ext().execute_with(|| {
		let network = &TEST_NETWORK;

		// Set the validators used to create the scp messages
		let (mut validators, mut validator_secret_keys) = create_dummy_validators();
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(Origin::root(), validators.clone()));

		// Remove validators from the quorum set to make it invalid
		// Remove the first two satoshipay validators
		validators.remove(0);
		validator_secret_keys.remove(0);
		validators.remove(1);
		validator_secret_keys.remove(1);

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, network);

		assert_noop!(
			SpacewalkRelay::validate_stellar_transaction(
				tx_envelope,
				scp_envelopes,
				tx_set,
				network
			),
			Error::<Test>::InvalidQuorumSet
		);
	});
}

#[test]
fn validate_stellar_transaction_works_for_correct_values() {
	new_test_ext().execute_with(|| {
		let network = &TEST_NETWORK;

		// Set the validators used to create the scp messages
		let (validators, validator_secret_keys) = create_dummy_validators();
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(Origin::root(), validators.clone()));

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, network);

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			tx_envelope,
			scp_envelopes,
			tx_set,
			network
		));
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
