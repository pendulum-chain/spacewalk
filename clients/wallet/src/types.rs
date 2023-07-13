use crate::horizon::responses::TransactionResponse;

pub type PagingToken = u128;

/// A filter trait to check whether `T` should be processed.
pub trait FilterWith<T: Clone, U: Clone> {
	/// logic to check whether a given param should be processed.
	fn is_relevant(&self, response: TransactionResponse, param_t: &T, param_u: &U) -> bool;
}
