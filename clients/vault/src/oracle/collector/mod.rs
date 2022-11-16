mod envelopes_handler;
mod proof_builder;

mod txsets_handler;

use parking_lot::{
	lock_api::{RwLockReadGuard, RwLockWriteGuard},
	RawRwLock, RwLock,
};
pub use proof_builder::*;
use std::{convert::TryInto, sync::Arc};
use stellar_relay::sdk::types::ScpStatementExternalize;

use crate::oracle::{
	errors::Error,
	types::{EnvelopesMap, Slot, SlotList, TxSetHash, TxSetHashAndSlotMap, TxSetMap},
};
use stellar_relay::sdk::network::{Network, PUBLIC_NETWORK, TEST_NETWORK};

/// Collects all ScpMessages and the TxSets.
pub struct ScpMessageCollector {
	/// holds the mapping of the Slot Number(key) and the ScpEnvelopes(value)
	envelopes_map: Arc<RwLock<EnvelopesMap>>,

	/// holds the mapping of the Slot Number(key) and the TransactionSet(value)
	txset_map: Arc<RwLock<TxSetMap>>,

	/// Mapping between the txset's hash and its corresponding slot.
	/// An entry is removed when a `TransactionSet` is found.
	txset_and_slot_map: Arc<RwLock<TxSetHashAndSlotMap>>,

	/// Holds the slots of transactions with `TransactionSet` already.
	/// A slot is removed when a proof is generated.
	slot_pendinglist: Arc<RwLock<Vec<Slot>>>,

	/// List of slots from transactions we want to generate a proof of.
	/// A slot is removed when a proof is generated.
	slot_watchlist: Arc<RwLock<SlotList>>,

	last_slot_index: Arc<RwLock<u64>>,

	public_network: bool,
	vault_addresses: Vec<String>,
}

impl ScpMessageCollector {
	pub(crate) fn new(public_network: bool, vault_addresses: Vec<String>) -> Self {
		ScpMessageCollector {
			envelopes_map: Default::default(),
			txset_map: Default::default(),
			txset_and_slot_map: Arc::new(Default::default()),
			slot_pendinglist: Arc::new(Default::default()),
			slot_watchlist: Arc::new(Default::default()),
			last_slot_index: Arc::new(Default::default()),
			public_network,
			vault_addresses,
		}
	}

	pub(crate) fn envelopes_map_len(&self) -> usize {
		self.envelopes_map.read().len()
	}

	fn envelopes_map(&self) -> RwLockReadGuard<'_, RawRwLock, EnvelopesMap> {
		self.envelopes_map.read()
	}

	fn envelopes_map_clone(&self) -> Arc<RwLock<EnvelopesMap>> {
		self.envelopes_map.clone()
	}

	fn txset_map(&self) -> RwLockReadGuard<'_, RawRwLock, TxSetMap> {
		self.txset_map.read()
	}

	fn txset_and_slot_map(&self) -> RwLockReadGuard<'_, RawRwLock, TxSetHashAndSlotMap> {
		self.txset_and_slot_map.read()
	}

	fn slot_pendinglist(&self) -> RwLockReadGuard<'_, RawRwLock, Vec<Slot>> {
		self.slot_pendinglist.read()
	}

	fn slot_watchlist(&self) -> RwLockReadGuard<'_, RawRwLock, SlotList> {
		self.slot_watchlist.read()
	}

	fn last_slot_index(&self) -> RwLockReadGuard<'_, RawRwLock, u64> {
		self.last_slot_index.read()
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

	fn txset_and_slot_map_mut(&mut self) -> RwLockWriteGuard<'_, RawRwLock, TxSetHashAndSlotMap> {
		self.txset_and_slot_map.write()
	}

	fn slot_pendinglist_mut(&self) -> RwLockWriteGuard<'_, RawRwLock, Vec<Slot>> {
		self.slot_pendinglist.write()
	}

	fn slot_watchlist_mut(&mut self) -> RwLockWriteGuard<'_, RawRwLock, SlotList> {
		self.slot_watchlist.write()
	}

	fn last_slot_index_mut(&mut self) -> RwLockWriteGuard<'_, RawRwLock, u64> {
		self.last_slot_index.write()
	}

	pub fn watch_transaction(&mut self, tx_from_horizon: crate::horizon::Transaction) {
		let slot = Slot::from(tx_from_horizon.ledger());

		let mut list = self.slot_watchlist_mut();
		list.insert(slot, ());
	}
}

pub fn get_tx_set_hash(x: &ScpStatementExternalize) -> Result<TxSetHash, Error> {
	let scp_value = x.commit.value.get_vec();
	scp_value[0..32].try_into().map_err(Error::from)
}
