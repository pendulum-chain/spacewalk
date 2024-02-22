use crate::horizon::responses::TransactionResponse;
use primitives::stellar::TransactionEnvelope;
use std::{collections::HashMap, fmt, fmt::Formatter};

pub type PagingToken = u128;
pub type Slot = u32;
pub type StatusCode = u16;
pub type LedgerTxEnvMap = HashMap<Slot, TransactionEnvelope>;

/// The child attributes of the `fee_charged` attribute of
/// [Fee Stats Object](https://developers.stellar.org/api/horizon/aggregations/fee-stats/object).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FeeAttribute {
	max,
	min,
	mode,
	p10,
	p20,
	p30,
	p40,
	p50,
	p60,
	p70,
	p80,
	p90,
	p95,
	p99,
}

impl fmt::Display for FeeAttribute {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl Default for FeeAttribute {
	fn default() -> Self {
		FeeAttribute::p95
	}
}

/// A filter trait to check whether `T` should be processed.
pub trait FilterWith<T: Clone, U: Clone> {
	/// logic to check whether a given param should be processed.
	fn is_relevant(&self, response: TransactionResponse, param_t: &T, param_u: &U) -> bool;
}
