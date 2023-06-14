use frame_support::{assert_noop, assert_ok, BoundedVec};
use sp_runtime::DispatchError::BadOrigin;
use substrate_stellar_sdk::{
	compound_types::{LimitedVarArray, LimitedVarOpaque, UnlimitedVarArray, UnlimitedVarOpaque},
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::{
		NodeId, Preconditions, ScpBallot, ScpEnvelope, ScpStatement, ScpStatementConfirm,
		ScpStatementExternalize, ScpStatementPledges, Signature, StellarValue, StellarValueExt,
		TransactionExt, TransactionSet, TransactionV1Envelope, Value,
	},
	Hash, Memo, MuxedAccount, PublicKey, SecretKey, Transaction, TransactionEnvelope, XdrCodec,
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

	ScpEnvelope { statement, signature }
}

fn create_scp_envelope(
	validator: &ValidatorOf<Test>,
	validator_secret_key: &SecretKey,
	network: &Network,
	pledges: ScpStatementPledges,
) -> ScpEnvelope {
	let node_id = NodeId::from_encoding(validator.public_key.clone()).unwrap();

	let statement: ScpStatement = ScpStatement { node_id, slot_index: 0, pledges };

	let network = network.get_id();
	let envelope_type_scp = [0, 0, 0, 1].to_vec(); // xdr representation
	let body: Vec<u8> = [network.to_vec(), envelope_type_scp, statement.to_xdr()].concat();
	let signature_result = validator_secret_key.create_signature(body);
	let signature: Signature = LimitedVarOpaque::new(signature_result.to_vec()).unwrap();

	ScpEnvelope { statement, signature }
}

fn create_valid_dummy_scp_envelopes(
	validators: Vec<ValidatorOf<Test>>,
	validator_secret_keys: Vec<SecretKey>,
	public_network: bool,
	num_externalized: usize, // number of externalized envelopes vs confirmed envelopes
) -> (TransactionEnvelope, TransactionSet, LimitedVarArray<ScpEnvelope, { i32::MAX }>) {
	// Build a transaction
	let source_account = MuxedAccount::from(PublicKey::PublicKeyTypeEd25519([0; 32]));
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

	let tx_set_hash = crate::compute_non_generic_tx_set_content_hash(&transaction_set)
		.expect("Should compute non generic tx set content hash");

	let network: &Network = if public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

	// Build the scp messages that externalize the transaction set
	// The scp messages have to be externalized by nodes that build a valid quorum set
	let mut envelopes = UnlimitedVarArray::<ScpEnvelope>::new_empty();

	// Build the value that is to be externalized
	let stellar_value: StellarValue = StellarValue {
		tx_set_hash,
		close_time: 0,
		ext: StellarValueExt::StellarValueBasic,
		upgrades: LimitedVarArray::new_empty(),
	};
	let stellar_value_xdr = stellar_value.to_xdr();
	let value: Value = UnlimitedVarOpaque::new(stellar_value_xdr).unwrap();

	for (i, validator_secret_key) in validator_secret_keys.iter().enumerate() {
		let validator = validators.get(i).unwrap();

		// We build the pledge depending on the number of externalized envelopes vs confirmed
		// envelopes
		let pledges = if i < num_externalized {
			// Build an externalize pledge
			ScpStatementPledges::ScpStExternalize(ScpStatementExternalize {
				commit: ScpBallot { counter: 0, value: value.clone() },
				n_h: 0,
				commit_quorum_set_hash: Hash::default(),
			})
		} else {
			// Build a confirmed pledge
			ScpStatementPledges::ScpStConfirm(ScpStatementConfirm {
				ballot: ScpBallot { counter: 0, value: value.clone() },
				n_prepared: 0,
				n_commit: 0,
				n_h: 0,
				quorum_set_hash: Hash::default(),
			})
		};
		let envelope = create_scp_envelope(validator, validator_secret_key, network, pledges);
		envelopes.push(envelope).expect("Should push envelope");
	}

	(transaction_envelope, transaction_set, envelopes)
}

#[test]
fn validate_stellar_transaction_fails_for_wrong_signature() {
	run_test(|_, validators, mut validator_secret_keys| {
		let public_network = true;

		// Change one of the secret keys, so that the signature is invalid
		validator_secret_keys[0] = SecretKey::from_binary([1; 32]);

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 2);

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
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
			LimitedVarArray::new(changed_envs).unwrap();

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &changed_env_array, &tx_set);
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
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 2);

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::EnvelopeSignedByUnknownValidator)));
	});
}

#[test]
fn validate_stellar_transaction_fails_for_wrong_transaction() {
	run_test(|_, validators, validator_secret_keys| {
		let public_network = true;

		let (_tx_envelope, mut tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 2);

		// Change tx_envelope that was used to create scp_envelopes
		let changed_tx_envelope = TransactionEnvelope::EnvelopeTypeTx(TransactionV1Envelope {
			tx: Transaction {
				source_account: MuxedAccount::from(PublicKey::PublicKeyTypeEd25519([1; 32])),
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
			&changed_tx_envelope,
			&scp_envelopes,
			&tx_set,
		);
		assert!(matches!(result, Err(Error::<Test>::TransactionNotInTransactionSet)));

		// Add transaction to transaction set
		tx_set.txes.push(changed_tx_envelope.clone()).unwrap();
		let result = SpacewalkRelay::validate_stellar_transaction(
			&changed_tx_envelope,
			&scp_envelopes,
			&tx_set,
		);
		assert!(matches!(result, Err(Error::<Test>::TransactionSetHashMismatch)));
	});
}

#[test]
fn validate_stellar_transaction_fails_when_using_the_same_validator_multiple_times() {
	run_test(|_, mut validators, mut validator_secret_keys| {
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
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 2);

		// This should be invalid because the quorum thresholds are based on distinct validators
		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::InvalidQuorumSetNotEnoughValidators)));
	});
}
#[test]
fn validate_stellar_transaction_fails_for_invalid_quorum() {
	run_test(|_, validators, validator_secret_keys| {
		let public_network = true;

		let mut drained_validators = validators.clone();
		let mut drained_validator_secret_keys = validator_secret_keys.clone();
		// Remove validators from the quorum set to make it invalid
		// Remove all sdf validators
		drained_validators.drain(0..3);
		drained_validator_secret_keys.drain(0..3);
		// Remove all keybase validators
		drained_validators.drain(0..3);
		drained_validator_secret_keys.drain(0..3);

		let (tx_envelope, tx_set, scp_envelopes) = create_valid_dummy_scp_envelopes(
			drained_validators.clone(),
			drained_validator_secret_keys.clone(),
			public_network,
			2,
		);

		// This should be an invalid quorum set because only 50% of the total organizations are in
		// the quorum set but it has to be >66%
		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::InvalidQuorumSetNotEnoughOrganizations)));

		// Reset variables
		let mut drained_validators = validators;
		let mut drained_validator_secret_keys = validator_secret_keys;
		// Remove validators from the quorum set to make it invalid
		// Remove two keybase validators
		drained_validators.drain(3..5);
		drained_validator_secret_keys.drain(3..5);
		// Remove two sdf validators
		drained_validators.drain(0..2);
		drained_validator_secret_keys.drain(0..2);

		let (tx_envelope, tx_set, scp_envelopes) = create_valid_dummy_scp_envelopes(
			drained_validators,
			drained_validator_secret_keys,
			public_network,
			2,
		);

		// This should be an invalid quorum set because 1/2 of the organizations only have 1/3 of
		// their validator nodes in the quorum set. This is not enough because >2/3 of the
		// organizations have to have >1/2 of their validator nodes to build a valid quorum set.
		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::InvalidQuorumSetNotEnoughValidators)));
	});
}

#[test]
fn validate_stellar_transaction_fails_for_differing_networks() {
	run_test(|_, validators, validator_secret_keys| {
		let (tx_envelope, tx_set, scp_envelopes) = create_valid_dummy_scp_envelopes(
			validators,
			validator_secret_keys,
			// Create scp messages for the test network although the relay is configured in
			// genesis to use main network
			false,
			2,
		);

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::InvalidEnvelopeSignature)));
	});
}

#[test]
fn validate_stellar_transaction_fails_without_validators() {
	run_test(|organizations, validators, validator_secret_keys| {
		let public_network = true;

		// Remove all validators
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			vec![],
			organizations,
			0
		));

		let (tx_envelope, tx_set, scp_envelopes) = create_valid_dummy_scp_envelopes(
			validators.clone(),
			validator_secret_keys,
			public_network,
			2,
		);

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::NoValidatorsRegistered)));

		// Remove all validators
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			validators,
			vec![],
			0
		));
		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::NoOrganizationsRegistered)));
	});
}

#[test]
fn validate_stellar_transaction_works_with_barely_enough_validators() {
	run_test(|_, mut validators, mut validator_secret_keys| {
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
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 2);

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			&tx_envelope,
			&scp_envelopes,
			&tx_set,
		));
	});
}

#[test]
fn validate_stellar_transaction_works_with_all_validators() {
	run_test(|_, validators, validator_secret_keys| {
		let public_network = true;
		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 2);

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			&tx_envelope,
			&scp_envelopes,
			&tx_set,
		));
	});
}

#[test]
fn update_tier_1_validator_set_fails_for_non_root_origin() {
	run_test(|_, _, _| {
		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(
				RuntimeOrigin::signed(1),
				vec![],
				vec![],
				0
			),
			BadOrigin
		);
	});
}

#[test]
fn update_tier_1_validator_set_works() {
	run_test(|_, _, _| {
		let organization = Organization { id: 0, name: Default::default() };
		let validator = Validator {
			name: Default::default(),
			public_key: BoundedVec::<u8, FieldLength>::try_from(vec![0u8; 128]).unwrap(),
			organization_id: organization.id,
		};
		let mut validator_1 = validator.clone();
		validator_1.public_key = BoundedVec::<u8, FieldLength>::try_from(vec![1u8; 128]).unwrap();
		let mut validator_2 = validator.clone();
		validator_2.public_key = BoundedVec::<u8, FieldLength>::try_from(vec![2u8; 128]).unwrap();
		let validator_set = vec![validator, validator_1.clone(), validator_2];
		let organization_set = vec![organization; 1];
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			validator_set.clone(),
			organization_set.clone(),
			0
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
			public_key: BoundedVec::<u8, FieldLength>::try_from(vec![0u8; 128]).unwrap(),
			organization_id: organization.id,
		};

		let new_validator_set = vec![validator, validator_1];
		let new_organization_set = vec![organization; 1];
		assert_ne!(validator_set, new_validator_set);
		assert_ne!(organization_set, new_organization_set);

		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			new_validator_set.clone(),
			new_organization_set.clone(),
			0
		));
		let validator_bounded_vec =
			BoundedVec::<ValidatorOf<Test>, ValidatorLimit>::try_from(new_validator_set).unwrap();
		let organization_bounded_vec =
			BoundedVec::<OrganizationOf<Test>, OrganizationLimit>::try_from(new_organization_set)
				.unwrap();
		assert_eq!(SpacewalkRelay::validators(), validator_bounded_vec);
		assert_eq!(SpacewalkRelay::organizations(), organization_bounded_vec);
	});
}

#[test]
fn update_tier_1_validator_set_fails_when_set_too_large() {
	run_test(|_, _, _| {
		let organization = Organization { id: 0, name: Default::default() };
		let validator = Validator {
			name: Default::default(),
			public_key: BoundedVec::<u8, FieldLength>::try_from(vec![0u8; 128]).unwrap(),
			organization_id: organization.id,
		};
		// 255 is configured as limit in the test runtime so we try 256
		let validator_set = vec![validator.clone(); 256];
		let organization_set = vec![organization.clone(); 3];
		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(
				RuntimeOrigin::root(),
				validator_set,
				organization_set,
				0
			),
			Error::<Test>::ValidatorLimitExceeded
		);

		// 255 is configured as limit in the test runtime so we try 256
		let validator_set = vec![validator; 3];
		let organization_set = vec![organization; 256];
		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(
				RuntimeOrigin::root(),
				validator_set,
				organization_set,
				0
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

	let is_valid = crate::verify_signature(&envelope, node_id, network);

	assert!(is_valid)
}

#[test]
fn verify_signature_works_for_mock_message() {
	let secret = SecretKey::from_binary([0; 32]);
	let network = &TEST_NETWORK;
	let envelope: ScpEnvelope = create_dummy_externalize_message(&secret, network);
	let node_id: &PublicKey = secret.get_public();

	let is_valid = crate::verify_signature(&envelope, node_id, network);

	assert!(is_valid)
}

#[test]
fn update_tier_1_validator_store_old_organization_and_validator_and_block_height_works() {
	run_test(|_, _, _| {
		let organization = Organization { id: 0, name: Default::default() };

		let validator = Validator {
			name: BoundedVec::<u8, FieldLength>::try_from(vec![1u8; 128]).unwrap(),
			public_key: BoundedVec::<u8, FieldLength>::try_from(vec![1u8; 128]).unwrap(),
			organization_id: organization.id,
		};
		let validator_set = vec![validator; 1];
		let organization_set = vec![organization; 1];
		let new_validators_enactment_block_height = 11;
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			validator_set.clone(),
			organization_set.clone(),
			0
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
		let validator_bounded_vec_old = validator_bounded_vec;
		let organization_bounded_vec_old = organization_bounded_vec;

		// Update the validator set
		let organization = Organization { id: 1, name: Default::default() };
		let validator = Validator {
			name: Default::default(),
			public_key: BoundedVec::<u8, FieldLength>::try_from(vec![1u8; 128]).unwrap(),
			organization_id: organization.id,
		};
		let mut validator_2 = validator.clone();
		validator_2.public_key = BoundedVec::<u8, FieldLength>::try_from(vec![2u8; 128]).unwrap();

		let new_validator_set = vec![validator, validator_2];
		let new_organization_set = vec![organization; 1];
		assert_ne!(validator_set, new_validator_set);
		assert_ne!(organization_set, new_organization_set);

		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			new_validator_set.clone(),
			new_organization_set.clone(),
			new_validators_enactment_block_height
		));
		let validator_bounded_vec =
			BoundedVec::<ValidatorOf<Test>, ValidatorLimit>::try_from(new_validator_set).unwrap();
		let organization_bounded_vec =
			BoundedVec::<OrganizationOf<Test>, OrganizationLimit>::try_from(new_organization_set)
				.unwrap();
		assert_eq!(SpacewalkRelay::validators(), validator_bounded_vec);
		assert_eq!(SpacewalkRelay::organizations(), organization_bounded_vec);

		assert_eq!(SpacewalkRelay::old_validators(), validator_bounded_vec_old);
		assert_eq!(SpacewalkRelay::old_organizations(), organization_bounded_vec_old);

		assert_eq!(
			SpacewalkRelay::new_validators_enactment_block_height(),
			new_validators_enactment_block_height
		);
	});
}

#[test]
fn validate_stellar_transaction_fails_no_validators_registered_when_new_validators_enactment_block_height_reached(
) {
	run_test(|_, validators, validator_secret_keys| {
		let public_network = true;
		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 2);

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			&tx_envelope,
			&scp_envelopes,
			&tx_set,
		));

		let validator_set: Vec<Validator<_>> = vec![];
		let organization_set: Vec<Organization<_>> = vec![];
		let new_validators_enactment_block_height = 11;
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			validator_set,
			organization_set,
			new_validators_enactment_block_height
		));

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			&tx_envelope,
			&scp_envelopes,
			&tx_set,
		));

		System::set_block_number(new_validators_enactment_block_height);

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::NoValidatorsRegistered)));
	});
}

#[test]
fn validate_stellar_transaction_fails_no_organizations_registered_when_new_validators_enactment_block_height_reached(
) {
	run_test(|_, validators, validator_secret_keys| {
		let validators_cloned = validators.clone();
		let public_network = true;
		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 2);

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			&tx_envelope,
			&scp_envelopes,
			&tx_set,
		));

		let empty_validator_set: Vec<Validator<_>> = vec![];
		let empty_organization_set: Vec<Organization<_>> = vec![];
		let mut new_validators_enactment_block_height = 11;
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			empty_validator_set,
			empty_organization_set.clone(),
			new_validators_enactment_block_height
		));

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			&tx_envelope,
			&scp_envelopes,
			&tx_set,
		));

		System::set_block_number(new_validators_enactment_block_height + 2);

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);

		assert!(matches!(result, Err(Error::<Test>::NoValidatorsRegistered)));

		new_validators_enactment_block_height *= 2;
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			validators_cloned,
			empty_organization_set,
			new_validators_enactment_block_height
		));

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::NoValidatorsRegistered)));

		System::set_block_number(new_validators_enactment_block_height);
		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);

		assert!(matches!(result, Err(Error::<Test>::NoOrganizationsRegistered)));
	});
}

#[test]
fn validate_stellar_transaction_works_when_enactment_block_height_reached() {
	run_test(|organizations, validators, validator_secret_keys| {
		let validators_cloned = validators.clone();
		let public_network = true;
		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 2);

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			&tx_envelope,
			&scp_envelopes,
			&tx_set,
		));

		let empty_validator_set: Vec<Validator<_>> = vec![];
		let empty_organization_set: Vec<Organization<_>> = vec![];
		let mut new_validators_enactment_block_height = 11;
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			empty_validator_set,
			empty_organization_set.clone(),
			new_validators_enactment_block_height
		));

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			&tx_envelope,
			&scp_envelopes,
			&tx_set,
		));

		System::set_block_number(new_validators_enactment_block_height);

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);

		assert!(matches!(result, Err(Error::<Test>::NoValidatorsRegistered)));

		new_validators_enactment_block_height *= 2;
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			validators_cloned.clone(),
			empty_organization_set,
			new_validators_enactment_block_height
		));

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::NoValidatorsRegistered)));

		System::set_block_number(new_validators_enactment_block_height);
		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);

		assert!(matches!(result, Err(Error::<Test>::NoOrganizationsRegistered)));

		new_validators_enactment_block_height *= 2;
		assert_ok!(SpacewalkRelay::update_tier_1_validator_set(
			RuntimeOrigin::root(),
			validators_cloned,
			organizations,
			new_validators_enactment_block_height
		));

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::NoOrganizationsRegistered)));

		System::set_block_number(new_validators_enactment_block_height);

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			&tx_envelope,
			&scp_envelopes,
			&tx_set,
		));
	});
}

#[test]
fn validate_stellar_transaction_fails_for_differing_slot_index() {
	run_test(|_, validators, validator_secret_keys| {
		let public_network = true;

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 2);

		// Change sequence number for the second envelope (we have to use at least the second
		// because the first envelope determines the expected slot index)
		let mut changed_envs = scp_envelopes.get_vec().clone();
		changed_envs[1].statement.slot_index += 1;
		let scp_envelopes: UnlimitedVarArray<ScpEnvelope> =
			LimitedVarArray::new(changed_envs).expect("Failed to create modified SCP envelopes");

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::EnvelopeSlotIndexMismatch)));
	});
}

#[test]
fn validate_stellar_transaction_fails_with_only_confirm_statements() {
	run_test(|_, validators, validator_secret_keys| {
		let public_network = true;

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 0);

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::MissingExternalizedMessage)));
	});
}

#[test]
fn validate_stellar_transaction_fails_with_mismatching_n_h() {
	run_test(|_, validators, validator_secret_keys| {
		let public_network = true;

		let (tx_envelope, tx_set, scp_envelopes) = create_valid_dummy_scp_envelopes(
			validators,
			validator_secret_keys.clone(),
			public_network,
			1,
		);

		// Change the n_h of the second envelope
		let mut second_env = scp_envelopes.get_vec()[1].clone();
		let changed_pledges: ScpStatementPledges = match second_env.statement.pledges {
			ScpStatementPledges::ScpStExternalize(mut ext) => {
				ext.n_h += 1;
				ScpStatementPledges::ScpStExternalize(ext)
			},
			ScpStatementPledges::ScpStConfirm(mut confirm) => {
				confirm.n_h += 1;
				ScpStatementPledges::ScpStConfirm(confirm)
			},
			_ => panic!("Expected externalize or confirm statement"),
		};
		second_env.statement.pledges = changed_pledges;

		// Create new signature
		let network = PUBLIC_NETWORK.get_id();
		let envelope_type_scp = [0, 0, 0, 1].to_vec(); // xdr representation of SCP envelope type
		let body: Vec<u8> =
			[network.to_vec(), envelope_type_scp, second_env.statement.to_xdr()].concat();
		let signature_result = validator_secret_keys[1].clone().create_signature(body);
		let signature: Signature = LimitedVarOpaque::new(signature_result.to_vec()).unwrap();
		second_env.signature = signature;

		let mut changed_envs = scp_envelopes.get_vec().clone();
		changed_envs[1] = second_env;

		let scp_envelopes: UnlimitedVarArray<ScpEnvelope> =
			LimitedVarArray::new(changed_envs).expect("Failed to create modified SCP envelopes");

		let result =
			SpacewalkRelay::validate_stellar_transaction(&tx_envelope, &scp_envelopes, &tx_set);
		assert!(matches!(result, Err(Error::<Test>::ExternalizedNHMismatch)));
	});
}

#[test]
fn validate_stellar_transaction_works_with_exactly_one_externalized_message() {
	run_test(|_, validators, validator_secret_keys| {
		let public_network = true;

		let (tx_envelope, tx_set, scp_envelopes) =
			create_valid_dummy_scp_envelopes(validators, validator_secret_keys, public_network, 1);

		assert_ok!(SpacewalkRelay::validate_stellar_transaction(
			&tx_envelope,
			&scp_envelopes,
			&tx_set
		));
	});
}

#[test]
fn update_tier_1_validator_set_fails_for_duplicate_values() {
	run_test(|_, _, _| {
		let organization = Organization { id: 0, name: Default::default() };

		let validator = Validator {
			name: BoundedVec::<u8, FieldLength>::try_from(vec![1u8; 128]).unwrap(),
			public_key: BoundedVec::<u8, FieldLength>::try_from(vec![1u8; 128]).unwrap(),
			organization_id: organization.id,
		};
		let validator_set = vec![validator.clone(); 2];
		let organization_set = vec![organization.clone(); 1];

		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(
				RuntimeOrigin::root(),
				validator_set.clone(),
				organization_set.clone(),
				0
			),
			Error::<Test>::DuplicateValidatorPublicKey
		);

		let validator_set = vec![validator; 1];
		let organization_set = vec![organization; 2];
		assert_noop!(
			SpacewalkRelay::update_tier_1_validator_set(
				RuntimeOrigin::root(),
				validator_set.clone(),
				organization_set.clone(),
				0
			),
			Error::<Test>::DuplicateOrganizationId
		);
	});
}
