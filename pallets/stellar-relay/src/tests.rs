use frame_support::{assert_noop, assert_ok, BoundedVec};
use rand::Rng;
use sp_runtime::DispatchError::BadOrigin;
use substrate_stellar_sdk::{
	compound_types::{LimitedVarArray, LimitedVarOpaque, UnlimitedVarArray, UnlimitedVarOpaque},
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::{
		NodeId, Preconditions, ScpBallot, ScpEnvelope, ScpStatement, ScpStatementExternalize,
		ScpStatementPledges, Signature, StellarValue, StellarValueExt, TransactionExt,
		TransactionSet, TransactionV1Envelope, Value,
	},
	AccountId, Hash, Memo, MuxedAccount, PublicKey, SecretKey, Transaction, TransactionEnvelope,
	XdrCodec,
};

use crate::{
	mock::*,
	traits::{FieldLength, Organization, Validator},
	types::{OrganizationOf, ValidatorOf},
	Error,
};

fn create_dummy_externalize_message(keypair: &SecretKey, network: &Network) -> ScpEnvelope {
	let value: Value = UnlimitedVarOpaque::new([0; 32].to_vec()).unwrap();
	let commit = ScpBallot { counter: 1, value };

	let externalize = ScpStatementExternalize { commit, n_h: 1, commit_quorum_set_hash: [0; 32] };

	let pledges = ScpStatementPledges::ScpStExternalize(externalize);

	let node_id = keypair.get_public().clone();
	let statement = ScpStatement { node_id, slot_index: 1u64, pledges };

	let network = network.get_id();
	let envelope_type_scp = [0, 0, 0, 1].to_vec(); // xdr representation of SCP envelope type
	let body: Vec<u8> = [network.to_vec(), envelope_type_scp, statement.to_xdr()].concat();

	let signature_result = keypair.create_signature(body);
	let signature: Signature = LimitedVarOpaque::new(signature_result.to_vec()).unwrap();

	let envelope = ScpEnvelope { statement: statement.clone(), signature: signature.clone() };

	envelope
}

fn create_scp_envelope(
	tx_set_hash: Hash,
	validator: &ValidatorOf<Test>,
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
	validators: Vec<ValidatorOf<Test>>,
	validator_secret_keys: Vec<SecretKey>,
	public_network: bool,
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

	let network: &Network = if public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

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
fn validate_stellar_transaction_fails_for_wrong_signature() {
	run_test(|organizations, validators, mut validator_secret_keys| {
		let public_network = true;

		// Change one of the secret keys, so that the signature is invalid
		validator_secret_keys[0] = SecretKey::from_binary([1; 32]);

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network);

		let result = SpacewalkRelay::validate_stellar_transaction(
			tx_envelope.clone(),
			scp_envelopes.clone(),
			tx_set.clone(),
		);
		assert!(matches!(result, Err(Error::<Test>::InvalidEnvelopeSignature)));

		// Change something in the envelope
		let changed_envs = scp_envelopes
			.get_vec()
			.iter()
			.map(|env| {
				let mut changed_env = env.clone();
				changed_env.statement.slot_index = u64::MAX;
				changed_env
			})
			.collect::<Vec<ScpEnvelope>>();

		let changed_env_array: UnlimitedVarArray<ScpEnvelope> =
			LimitedVarArray::new(changed_envs.clone()).unwrap();

		let result = SpacewalkRelay::validate_stellar_transaction(
			tx_envelope.clone(),
			changed_env_array.clone(),
			tx_set.clone(),
		);
		assert!(matches!(result, Err(Error::<Test>::InvalidEnvelopeSignature)));
	});
}

#[test]
fn validate_stellar_transaction_fails_for_unknown_validator() {
	run_test(|organizations, mut validators, mut validator_secret_keys| {
		let public_network = true;

		// Add other validator that is not part of the 'known' validator set
		let (validator, validator_secret) = create_dummy_validator("$unknown", &organizations[0]);
		validators.push(validator);
		validator_secret_keys.push(validator_secret);

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network);

		let result =
			SpacewalkRelay::validate_stellar_transaction(tx_envelope, scp_envelopes, tx_set);
		assert!(matches!(result, Err(Error::<Test>::EnvelopeSignedByUnknownValidator)));
	});
}

#[test]
fn validate_stellar_transaction_fails_for_wrong_transaction() {
	run_test(|organizations, validators, validator_secret_keys| {
		let public_network = true;

		let (_tx_envelope, mut tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network);

		// Change tx_envelope that was used to create scp_envelopes
		let changed_tx_envelope = TransactionEnvelope::EnvelopeTypeTx(TransactionV1Envelope {
			tx: Transaction {
				source_account: MuxedAccount::from(AccountId::from(
					PublicKey::PublicKeyTypeEd25519([1; 32]),
				)),
				fee: 1,
				seq_num: 1,
				cond: Preconditions::PrecondNone,
				memo: Memo::MemoNone,
				operations: LimitedVarArray::new_empty(),
				ext: TransactionExt::V0,
			},
			signatures: LimitedVarArray::new(vec![]).unwrap(),
		});

		let result = SpacewalkRelay::validate_stellar_transaction(
			changed_tx_envelope.clone(),
			scp_envelopes.clone(),
			tx_set.clone(),
		);
		assert!(matches!(result, Err(Error::<Test>::TransactionNotInTransactionSet)));

		// Add transaction to transaction set
		tx_set.txes.push(changed_tx_envelope.clone()).unwrap();
		let result = SpacewalkRelay::validate_stellar_transaction(
			changed_tx_envelope,
			scp_envelopes,
			tx_set,
		);
		assert!(matches!(result, Err(Error::<Test>::TransactionSetHashMismatch)));
	});
}

#[test]
fn validate_stellar_transaction_fails_when_using_the_same_validator_multiple_times() {
	run_test(|organizations, mut validators, mut validator_secret_keys| {
		let public_network = true;

		// Modify validator list to use the same validator multiple times
		// Remove all sdf validators
		let sdf_validators = validators.drain(0..3).collect::<Vec<ValidatorOf<Test>>>();
		let sdf_validator_secret_keys =
			validator_secret_keys.drain(0..3).collect::<Vec<SecretKey>>();
		// Pick first removed sdf validator to be re-used
		let reused_validator = sdf_validators.get(0).unwrap();
		let reused_validator_secret_key = sdf_validator_secret_keys.get(0).unwrap();

		// Add the same sdf validator back to the list three times
		// -> the same sdf validator is used 3 times to sign the scp messages
		validators.push(reused_validator.clone());
		validator_secret_keys.push(reused_validator_secret_key.clone());
		validators.push(reused_validator.clone());
		validator_secret_keys.push(reused_validator_secret_key.clone());
		validators.push(reused_validator.clone());
		validator_secret_keys.push(reused_validator_secret_key.clone());

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network);

		// This should be invalid because the quorum thresholds are based on distinct validators
		let result =
			SpacewalkRelay::validate_stellar_transaction(tx_envelope, scp_envelopes, tx_set);
		assert!(matches!(result, Err(Error::<Test>::InvalidQuorumSetNotEnoughValidators)));
	});
}
#[test]
fn validate_stellar_transaction_fails_for_invalid_quorum() {
	run_test(|organizations, mut validators, mut validator_secret_keys| {
		let public_network = true;

		// Remove validators from the quorum set to make it invalid
		// Remove all sdf validators
		validators.drain(0..3);
		validator_secret_keys.drain(0..3);
		// Remove all keybase validators
		validators.drain(0..3);
		validator_secret_keys.drain(0..3);

		let (tx_envelope, tx_set, scp_envelopes) = create_valid_dummy_scp_envelopes(
			validators.clone(),
			validator_secret_keys.clone(),
			public_network,
		);

		// This should be an invalid quorum set because only 50% of the total organizations are in
		// the quorum set but it has to be >66%
		let result =
			SpacewalkRelay::validate_stellar_transaction(tx_envelope, scp_envelopes, tx_set);
		println!("{:?}", result);
		assert!(matches!(result, Err(Error::<Test>::InvalidQuorumSetNotEnoughOrganizations)));

		// Remove validators from the quorum set to make it invalid
		// Remove two keybase validators
		validators.drain(3..5);
		validator_secret_keys.drain(3..5);
		// Remove two sdf validators
		validators.drain(0..2);
		validator_secret_keys.drain(0..2);

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network);

		// This should be an invalid quorum set because 1/2 of the organizations only have 1/3 of
		// their validator nodes in the quorum set. This is not enough because >2/3 of the
		// organizations have to have >1/2 of their validator nodes to build a valid quorum set.
		let result =
			SpacewalkRelay::validate_stellar_transaction(tx_envelope, scp_envelopes, tx_set);
		println!("{:?}", result);
		assert!(matches!(result, Err(Error::<Test>::InvalidQuorumSetNotEnoughValidators)));
	});
}

#[test]
fn validate_stellar_transaction_fails_for_differing_networks() {
	run_test(|organizations, validators, validator_secret_keys| {
		let (tx_envelope, tx_set, scp_envelopes) = create_valid_dummy_scp_envelopes(
			validators,
			validator_secret_keys,
			// Create scp messages for the test network although the relay is configured in genesis
			// to use public network
			false,
		);

		let result =
			SpacewalkRelay::validate_stellar_transaction(tx_envelope, scp_envelopes, tx_set);
		assert!(matches!(result, Err(Error::<Test>::InvalidEnvelopeSignature)));
	});
}

#[test]
fn validate_stellar_transaction_fails_without_validators() {
	run_test(|organizations, validators, validator_secret_keys| {
		let public_network = true;

		// Remove all validators
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			Origin::root(),
			vec![],
			organizations,
		));

		let (tx_envelope, tx_set, scp_envelopes) = create_valid_dummy_scp_envelopes(
			validators.clone(),
			validator_secret_keys,
			public_network,
		);

		let result = SpacewalkRelay::validate_stellar_transaction(
			tx_envelope.clone(),
			scp_envelopes.clone(),
			tx_set.clone(),
		);
		assert!(matches!(result, Err(Error::<Test>::NoValidatorsRegistered)));

		// Remove all validators
		assert_ok!(
			SpacewalkRelay::update_tier_1_validator_set(Origin::root(), validators, vec![],)
		);
		let result =
			SpacewalkRelay::validate_stellar_transaction(tx_envelope, scp_envelopes, tx_set);
		assert!(matches!(result, Err(Error::<Test>::NoOrganizationsRegistered)));
	});
}

#[test]
fn validate_stellar_transaction_works_with_barely_enough_validators() {
	run_test(|organizations, mut validators, mut validator_secret_keys| {
		let public_network = true;

		// Remove some validators but leave enough to build a valid quorum set
		// Remove all sdf validators
		validators.drain(0..3);
		validator_secret_keys.drain(0..3);
		// Remove one keybase validator
		validators.remove(0);
		validator_secret_keys.remove(0);
		// Remove one satoshipay validator
		validators.remove(3);
		validator_secret_keys.remove(3);

		// This should still be valid because 3 out of 4 organizations are still present
		// and all organizations still have more than 1/2 of its validators in the quorum set

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network);

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			tx_envelope,
			scp_envelopes,
			tx_set,
		));
	});
}

#[test]
fn validate_stellar_transaction_works_with_all_validators() {
	run_test(|organizations, validators, validator_secret_keys| {
		let public_network = true;
		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network);

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			tx_envelope,
			scp_envelopes,
			tx_set,
		));
	});
}

#[test]
fn update_tier_1_validator_set_fails_for_non_root_origin() {
	run_test(|organizations, validators, validator_secret_keys| {
		// Ensure the expected error is thrown when no value is present.
		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(Origin::signed(1), vec![], vec![]),
			BadOrigin
		);
	});
}

#[test]
fn update_tier_1_validator_set_works() {
	run_test(|organizations, validators, validator_secret_keys| {
		let public_network = true;
		let organization = Organization { id: 0, name: Default::default() };
		let validator = Validator {
			name: Default::default(),
			public_key: Default::default(),
			organization_id: organization.id,
		};
		let validator_set = vec![validator; 3];
		let organization_set = vec![organization; 3];
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			Origin::root(),
			validator_set.clone(),
			organization_set.clone(),
		));

		let validator_bounded_vec =
			BoundedVec::<ValidatorOf<Test>, ValidatorLimit>::try_from(validator_set.clone())
				.unwrap();
		let organization_bounded_vec =
			BoundedVec::<OrganizationOf<Test>, OrganizationLimit>::try_from(
				organization_set.clone(),
			)
			.unwrap();
		assert_eq!(SpacewalkRelay::validators(), validator_bounded_vec);
		assert_eq!(SpacewalkRelay::organizations(), organization_bounded_vec);

		// Update the validator set
		let organization = Organization { id: 1, name: Default::default() };
		let validator = Validator {
			name: Default::default(),
			public_key: Default::default(),
			organization_id: organization.id,
		};
		let new_validator_set = vec![validator; 2];
		let new_organization_set = vec![organization; 2];
		// let new_validator_set: Vec<ValidatorOf<Test>> = vec![validator; 2];
		// let new_organization_set: Vec<OrganizationOf<Test>> = vec![organization; 2];
		assert_ne!(validator_set, new_validator_set);
		assert_ne!(organization_set, new_organization_set);

		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			Origin::root(),
			new_validator_set.clone(),
			new_organization_set.clone(),
		));
		let validator_bounded_vec =
			BoundedVec::<ValidatorOf<Test>, ValidatorLimit>::try_from(new_validator_set.clone())
				.unwrap();
		let organization_bounded_vec =
			BoundedVec::<OrganizationOf<Test>, OrganizationLimit>::try_from(
				new_organization_set.clone(),
			)
			.unwrap();
		assert_eq!(SpacewalkRelay::validators(), validator_bounded_vec);
		assert_eq!(SpacewalkRelay::organizations(), organization_bounded_vec);
	});
}

#[test]
fn update_tier_1_validator_set_fails_when_set_too_large() {
	run_test(|organizations, validators, validator_secret_keys| {
		let public_network = true;
		let organization = Organization { id: 0, name: Default::default() };
		let validator = Validator {
			name: Default::default(),
			public_key: Default::default(),
			organization_id: organization.id,
		};
		// 255 is configured as limit in the test runtime so we try 256
		let validator_set = vec![validator.clone(); 256];
		let organization_set = vec![organization.clone(); 3];
		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(
				Origin::root(),
				validator_set,
				organization_set,
			),
			Error::<Test>::ValidatorLimitExceeded
		);

		// 255 is configured as limit in the test runtime so we try 256
		let validator_set = vec![validator.clone(); 3];
		let organization_set = vec![organization.clone(); 256];
		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(
				Origin::root(),
				validator_set,
				organization_set,
			),
			Error::<Test>::OrganizationLimitExceeded
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
	let secret = SecretKey::from_binary([0; 32]);
	let network = &TEST_NETWORK;
	let envelope: ScpEnvelope = create_dummy_externalize_message(&secret, network);
	let node_id: &PublicKey = secret.get_public();

	let is_valid = crate::verify_signature(&envelope, &node_id, network);

	assert!(is_valid)
}
