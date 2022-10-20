mod envelopes_handler;
mod proof_builder;
mod tx_handler;
mod txsets_handler;

use parking_lot::{
	lock_api::{RwLockReadGuard, RwLockWriteGuard},
	RawRwLock, RwLock,
};
pub use proof_builder::*;
use std::{convert::TryInto, sync::Arc};
use stellar_relay::sdk::{types::ScpStatementExternalize, Memo, TransactionEnvelope};

use crate::oracle::{
	errors::Error,
	types::{EnvelopesMap, Slot, TxHashMap, TxSetHash, TxSetMap},
};
use stellar_relay::sdk::network::{Network, PUBLIC_NETWORK, TEST_NETWORK};

/// Collects all ScpMessages and the TxSets.
pub struct ScpMessageCollector {
	/// holds the mapping of the Slot Number(key) and the ScpEnvelopes(value)
	envelopes_map: Arc<RwLock<EnvelopesMap>>,
	/// holds the mapping of the Slot Number(key) and the TransactionSet(value)
	txset_map: Arc<RwLock<TxSetMap>>,
	/// holds the mapping of the Transaction Hash(key) and the Slot Number(value)
	tx_hash_map: Arc<RwLock<TxHashMap>>,
	/// Holds the transactions that still have to be processed but were not because not enough scp
	/// messages are available yet.
	pending_transactions: Vec<(TransactionEnvelope, Slot)>,

	public_network: bool,
	vault_addresses: Vec<String>,
}

impl ScpMessageCollector {
	pub(crate) fn new(public_network: bool, vault_addresses: Vec<String>) -> Self {
		ScpMessageCollector {
			envelopes_map: Default::default(),
			txset_map: Default::default(),
			tx_hash_map: Default::default(),
			pending_transactions: vec![],
			public_network,
			vault_addresses,
		}
	}

	fn envelopes_map(&self) -> RwLockReadGuard<'_, RawRwLock, EnvelopesMap> {
		self.envelopes_map.read()
	}

	pub(crate) fn envelopes_map_len(&self) -> usize {
		self.envelopes_map.read().len()
	}

	fn txset_map(&self) -> RwLockReadGuard<'_, RawRwLock, TxSetMap> {
		self.txset_map.read()
	}

	fn tx_hash_map(&self) -> RwLockReadGuard<'_, RawRwLock, TxHashMap> {
		self.tx_hash_map.read()
	}

	pub fn network(&self) -> &Network {
		if self.public_network {
			&PUBLIC_NETWORK
		} else {
			&TEST_NETWORK
		}
	}

	pub fn is_public(&self) -> bool {
		self.public_network
	}

	fn envelopes_map_mut(&mut self) -> RwLockWriteGuard<'_, RawRwLock, EnvelopesMap> {
		self.envelopes_map.write()
	}

	fn txset_map_mut(&mut self) -> RwLockWriteGuard<'_, RawRwLock, TxSetMap> {
		self.txset_map.write()
	}

	fn tx_hash_map_mut(&mut self) -> RwLockWriteGuard<'_, RawRwLock, TxHashMap> {
		self.tx_hash_map.write()
	}
}

pub fn get_tx_set_hash(x: &ScpStatementExternalize) -> Result<TxSetHash, Error> {
	let scp_value = x.commit.value.get_vec();
	scp_value[0..32].try_into().map_err(Error::from)
}

/// We only save transactions that has Memohash.
fn is_hash_memo(memo: &Memo) -> bool {
	match memo {
		Memo::MemoHash(_) => true,
		_ => false,
	}
}
