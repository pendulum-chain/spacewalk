use crate::horizon::responses::TransactionResponse;
use primitives::stellar::TransactionEnvelope;
use std::collections::HashMap;

pub type PagingToken = u128;
pub type Slot = u32;
pub type StatusCode = u16;
pub type LedgerTxEnvMap = HashMap<Slot, TransactionEnvelope>;

/// A filter trait to check whether `T` should be processed.
pub trait FilterWith<T: Clone, U: Clone> {
	/// logic to check whether a given param should be processed.
	fn is_relevant(&self, response: TransactionResponse, param_t: &T, param_u: &U) -> bool;
}
