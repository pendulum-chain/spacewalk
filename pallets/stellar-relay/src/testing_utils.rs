use substrate_stellar_sdk::{
	compound_types::LimitedVarArray,
	types::{ScpEnvelope, TransactionSet, TransactionV1Envelope},
	MuxedAccount, Transaction, XdrCodec,
};

use primitives::StellarPublicKeyRaw;

pub const RANDOM_STELLAR_PUBLIC_KEY: StellarPublicKeyRaw = [0u8; 32];
pub const DEFAULT_STELLAR_PUBLIC_KEY: StellarPublicKeyRaw = [1u8; 32];

pub fn create_dummy_scp_structs(
) -> (TransactionV1Envelope, LimitedVarArray<ScpEnvelope, 20>, TransactionSet) {
	let tx = Transaction {
		source_account: MuxedAccount::KeyTypeEd25519(RANDOM_STELLAR_PUBLIC_KEY),
		fee: 100,
		seq_num: 1,
		operations: LimitedVarArray::new(vec![]).unwrap(),
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
