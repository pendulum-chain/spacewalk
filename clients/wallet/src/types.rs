pub use crate::horizon::TransactionResponse;

pub type StellarPublicKeyRaw = [u8; 32];

/// A filter trait to check whether `T` should be processed.
pub trait FilterWith<T: Clone, U: Clone> {
	/// logic to check whether a given param should be processed.
	fn is_relevant(&self, response: TransactionResponse, param_t: &T, param_u: &U) -> bool;
}
