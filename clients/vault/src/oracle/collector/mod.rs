mod collector;
mod handler;
mod proof_builder;

pub use collector::ScpMessageCollector;
pub use proof_builder::*;
use std::convert::TryInto;
use stellar_relay::sdk::types::ScpStatementExternalize;

use crate::oracle::{errors::Error, types::TxSetHash};

pub fn get_tx_set_hash(x: &ScpStatementExternalize) -> Result<TxSetHash, Error> {
	let scp_value = x.commit.value.get_vec();
	scp_value[0..32].try_into().map_err(Error::from)
}
