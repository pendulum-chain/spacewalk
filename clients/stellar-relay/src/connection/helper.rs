use rand::Rng;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use substrate_stellar_sdk::{
    types::{TransactionSet, Uint256},
    SecretKey, XdrCodec,
};

/// Returns a new BigNumber with a pseudo-random value equal to or greater than 0 and less than 1.
pub fn generate_random_nonce() -> Uint256 {
    let mut rng = rand::thread_rng();
    let random_float = rng.gen_range(0.00..1.00);
    let mut hash = Sha256::new();
    hash.update(random_float.to_string());
    hash.finalize().to_vec().try_into().unwrap()
}

pub fn secret_key_binary(key: &str) -> [u8; 32] {
    let bytes = base64::decode_config(key, base64::STANDARD).unwrap();
    let secret_key = SecretKey::from_binary(bytes.try_into().unwrap());
    secret_key.into_binary()
}

pub fn time_now() -> u64 {
    let valid_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    u64::try_from(valid_at).unwrap_or_else(|_| {
        log::warn!("could not convert time at u128 to u64.");
        u64::MAX
    })
}

//todo: this has to be moved somewhere.
pub fn compute_non_generic_tx_set_content_hash(tx_set: &TransactionSet) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(tx_set.previous_ledger_hash);

    tx_set.txes.get_vec().iter().for_each(|envlp| {
        hasher.update(envlp.to_xdr());
    });

    hasher.finalize().as_slice().try_into().unwrap()
}
