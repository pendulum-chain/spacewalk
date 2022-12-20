use substrate_stellar_sdk::Hash;

use crate::horizon::TransactionResponse;

pub type StellarPublicKeyRaw = [u8; 32];

pub type TransactionFilterParam<T> = (TransactionResponse, T);

/// A filter trait to check whether `T` should be processed.
pub trait FilterWith<T: Clone> {
	/// logic to check whether a given param should be processed.
	fn is_relevant(&self, param: T) -> bool;
}

#[derive(Clone)]
pub struct TxFilterByHash;

impl FilterWith<(TransactionResponse, Vec<Hash>)> for TxFilterByHash {
	fn is_relevant(&self, param: (TransactionResponse, Vec<Hash>)) -> bool {
		match String::from_utf8(param.0.memo_type.clone()) {
			Ok(memo_type) if memo_type == "hash" =>
				if let Some(memo) = &param.0.memo {
					return param.1.iter().any(|hash| &hash.to_vec() == memo)
				},
			Err(e) => {
				tracing::error!("Failed to retrieve memo type: {:?}", e);
			},
			_ => {},
		}
		false
	}
}
