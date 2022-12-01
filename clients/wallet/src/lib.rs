pub use stellar_wallet::{StellarWallet, Watcher};
use substrate_stellar_sdk::Hash;

pub mod error;
mod horizon;
mod stellar_wallet;

use crate::horizon::Transaction;
pub use horizon::listen_for_new_transactions;

/// The FilterWith has to be Send and Sync, as it is sent between channels.
pub type FilterTypes = (Transaction, Vec<Hash>);

/// A filter trait to check whether `T` should be processed.
pub trait FilterWith<T: Clone> {
	/// logic to check whether a given param should be processed.
	fn is_relevant(&self, param: T) -> bool;
}

#[derive(Clone)]
pub struct TxFilter;

impl FilterWith<(Transaction, Vec<Hash>)> for TxFilter {
	fn is_relevant(&self, param: (Transaction, Vec<Hash>)) -> bool {
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
