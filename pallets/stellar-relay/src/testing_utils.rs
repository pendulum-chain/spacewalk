use frame_support::BoundedVec;
use sp_std::{vec, vec::Vec};
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

use primitives::{StellarPublicKeyRaw, H256};

use crate::{
	traits::{Organization, Validator},
	types::{OrganizationOf, ValidatorOf},
};

pub const RANDOM_STELLAR_PUBLIC_KEY: StellarPublicKeyRaw = [0u8; 32];
pub const DEFAULT_STELLAR_PUBLIC_KEY: StellarPublicKeyRaw = [1u8; 32];

const VALIDATOR_1_SECRET: [u8; 32] = [1u8; 32];
const VALIDATOR_2_SECRET: [u8; 32] = [2u8; 32];
const VALIDATOR_3_SECRET: [u8; 32] = [3u8; 32];

pub fn create_dummy_scp_structs(
) -> (TransactionV1Envelope, LimitedVarArray<ScpEnvelope, 20>, TransactionSet) {
	let tx = Transaction {
		source_account: MuxedAccount::KeyTypeEd25519(RANDOM_STELLAR_PUBLIC_KEY),
		fee: 100,
		seq_num: 1,
		operations: LimitedVarArray::new_empty(),
		cond: substrate_stellar_sdk::types::Preconditions::PrecondNone,
		memo: substrate_stellar_sdk::Memo::MemoNone,
		ext: substrate_stellar_sdk::types::TransactionExt::V0,
	};
	let tx_env = TransactionV1Envelope { tx, signatures: LimitedVarArray::new_empty() };

	let scp_envelopes: LimitedVarArray<ScpEnvelope, 20> = LimitedVarArray::new_empty();

	let transaction_set = TransactionSet {
		previous_ledger_hash: Default::default(),
		txes: LimitedVarArray::new_empty(),
	};

	(tx_env, scp_envelopes, transaction_set)
}

/// This function is to be used by other crates which mock the validation function
/// and don't necessarily needs valid scp structs
pub fn create_dummy_scp_structs_encoded() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
	let (tx_env, scp_envelopes, transaction_set) = create_dummy_scp_structs();
	let tx_env_encoded = base64::encode(tx_env.to_xdr()).as_bytes().to_vec();
	let scp_envelopes_encoded = base64::encode(scp_envelopes.to_xdr()).as_bytes().to_vec();
	let transaction_set_encoded = base64::encode(transaction_set.to_xdr()).as_bytes().to_vec();
	(tx_env_encoded, scp_envelopes_encoded, transaction_set_encoded)
}

fn create_scp_envelope(
	tx_set_hash: Hash,
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

	let node_id = NodeId::from_encoding(validator_secret_key.get_public().to_encoding()).unwrap();

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

pub fn get_validators_and_organizations<T: crate::Config>(
) -> (Vec<ValidatorOf<T>>, Vec<OrganizationOf<T>>) {
	// Validators and organizations needed to build valid proof in the benchmark
	let organization: OrganizationOf<T> = Organization {
		id: 1.into(),
		name: BoundedVec::try_from("organization".as_bytes().to_vec()).unwrap(),
		public_network: false,
	};

	let validator_1: ValidatorOf<T> = Validator {
		name: Default::default(),
		public_key: BoundedVec::try_from(
			SecretKey::from_binary(VALIDATOR_1_SECRET).get_public().to_encoding(),
		)
		.unwrap(),
		organization_id: organization.id,
		public_network: false,
	};
	let validator_2: ValidatorOf<T> = Validator {
		name: Default::default(),
		public_key: BoundedVec::try_from(
			SecretKey::from_binary(VALIDATOR_2_SECRET).get_public().to_encoding(),
		)
		.unwrap(),
		organization_id: organization.id,
		public_network: false,
	};
	let validator_3: ValidatorOf<T> = Validator {
		name: Default::default(),
		public_key: BoundedVec::try_from(
			SecretKey::from_binary(VALIDATOR_3_SECRET).get_public().to_encoding(),
		)
		.unwrap(),
		organization_id: organization.id,
		public_network: false,
	};

	(vec![validator_1, validator_2, validator_3], vec![organization])
}

pub fn build_dummy_proof_for<T: crate::Config>(
	issue_id: H256,
	public_network: bool,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
	// Build a transaction
	let source_account =
		MuxedAccount::from(AccountId::from(PublicKey::PublicKeyTypeEd25519([0; 32])));
	let operations = LimitedVarArray::new_empty();
	let transaction = Transaction {
		source_account,
		fee: 0,
		seq_num: 0,
		cond: Preconditions::PrecondNone,
		memo: Memo::MemoHash(Hash::from(issue_id)), // Include the issue id in the memo
		operations,
		ext: TransactionExt::V0,
	};

	let transaction_envelope: TransactionEnvelope =
		TransactionEnvelope::EnvelopeTypeTx(TransactionV1Envelope {
			tx: transaction,
			signatures: LimitedVarArray::new_empty(),
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
	let validator_secret_keys = vec![VALIDATOR_1_SECRET, VALIDATOR_2_SECRET, VALIDATOR_3_SECRET];
	for validator_secret_key in validator_secret_keys.iter() {
		let secret_key = SecretKey::from_binary(*validator_secret_key);
		let envelope = create_scp_envelope(tx_set_hash.clone(), &secret_key, network);
		envelopes.push(envelope).unwrap();
	}

	let tx_env_xdr_encoded = base64::encode(&transaction_envelope.to_xdr()).as_bytes().to_vec();
	let scp_envs_xdr_encoded = base64::encode(&envelopes.to_xdr()).as_bytes().to_vec();
	let tx_set_xdr_encoded = base64::encode(&transaction_set.to_xdr()).as_bytes().to_vec();
	(tx_env_xdr_encoded, scp_envs_xdr_encoded, tx_set_xdr_encoded)
}
