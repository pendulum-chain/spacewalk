use parking_lot::{lock_api::RwLockReadGuard, RawRwLock, RwLock};

use std::sync::Arc;
use stellar_relay::sdk::types::{ScpEnvelope, TransactionSet};

use crate::oracle::types::{
	EnvelopesMap, Slot, SlotList, TxSetHash, TxSetHashAndSlotMap, TxSetMap,
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
	/// This is a HashMap datatype, to avoid any duplicate records.
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

	pub fn envelopes_map_len(&self) -> usize {
		self.envelopes_map.read().len()
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

	/// watch out in Stellar Node's messages for SCPMessages containing this slot.
	pub fn watch_slot(&mut self, slot: Slot) {
		tracing::info!("watching slot {:?}", slot);
		self.slot_watchlist.write().insert(slot, ());
	}
}

// read functions
impl ScpMessageCollector {
	pub(super) fn envelopes_map(&self) -> RwLockReadGuard<'_, RawRwLock, EnvelopesMap> {
		self.envelopes_map.read()
	}

	pub(super) fn envelopes_map_clone(&self) -> Arc<RwLock<EnvelopesMap>> {
		self.envelopes_map.clone()
	}

	pub(super) fn txset_map(&self) -> RwLockReadGuard<'_, RawRwLock, TxSetMap> {
		self.txset_map.read()
	}

	pub(super) fn get_txset_hash(&self, slot: &Slot) -> Option<TxSetHash> {
		self.txset_and_slot_map.read().get_txset_hash(slot).cloned()
	}

	pub(super) fn read_slot_pending_list(&self) -> Vec<Slot> {
		self.slot_pendinglist.read().clone()
	}

	pub(crate) fn last_slot_index(&self) -> RwLockReadGuard<'_, RawRwLock, u64> {
		self.last_slot_index.read()
	}

	pub(crate) fn get_slot_watch_list(&self) -> Vec<Slot> {
		let res = self.slot_watchlist.read().clone();
		res.keys().cloned().collect::<Vec<Slot>>()
	}
}

// insert/add/save functions
impl ScpMessageCollector {
	pub(super) fn add_scp_envelope(&mut self, slot: Slot, scp_envelope: ScpEnvelope) {
		// insert/add the externalized message to map.
		let mut envelopes_map = self.envelopes_map.write();

		if let Some(value) = envelopes_map.get_mut(&slot) {
			value.push(scp_envelope);
		} else {
			tracing::debug!("Adding received SCP envelopes for slot {}", slot);
			envelopes_map.insert(slot, vec![scp_envelope]);
		}
	}

	pub(super) fn add_txset(&mut self, slot: Slot, transaction_set: TransactionSet) {
		self.txset_map.write().insert(slot, transaction_set);
	}

	pub(super) fn save_txset_hash_and_slot(&mut self, txset_hash: TxSetHash, slot: Slot) {
		// save the mapping of the hash of the txset and the slot.
		let mut m = self.txset_and_slot_map.write();
		m.insert(txset_hash, slot);
	}

	/// Once a TransactionSet is found, remove that entry in `txset_and_slot` map and then
	/// add to pending list.
	pub(super) fn insert_to_pending_list(&mut self, txset_hash: &TxSetHash) {
		match self.txset_and_slot_map.write().remove_by_txset_hash(txset_hash) {
			None => {
				tracing::warn!("WARNING! tx_set_hash: {:?} has no slot.", txset_hash);
			},
			Some(slot) => {
				self.slot_pendinglist.write().push(slot);
			},
		}
	}

	pub(super) fn set_last_slot_index(&mut self, slot: Slot) {
		let mut last_slot_index = self.last_slot_index.write();
		if slot > *last_slot_index {
			*last_slot_index = slot;
		}
	}
}

// delete/remove functions
impl ScpMessageCollector {
	/// Clear out data related to this slot.
	pub(super) fn remove_data(&mut self, slot: &Slot) {
		self.slot_watchlist.write().remove(slot);
		self.envelopes_map.write().remove(slot);
		self.txset_map.write().remove(slot);

		let mut to_remove = vec![];
		if let Some(idx) = self.slot_pendinglist.read().iter().position(|s| s == slot) {
			to_remove.push(idx);
		}

		to_remove.into_iter().for_each(|idx| {
			self.slot_pendinglist.write().remove(idx);
		});
	}
}

// checking and other implementations
impl ScpMessageCollector {
	/// checks whether the txset hash and slot tandem is already recorded/noted/flagged.
	pub(super) fn is_txset_new(&self, txset_hash: &TxSetHash, slot: &Slot) -> bool {
		self.txset_and_slot_map.read().get_slot(txset_hash).is_none() &&
            // also check whether this is a delayed message
            !self.txset_map.read().contains_key(slot)
	}

	/// checks whether the slot is on the watchlist.
	pub(super) fn is_slot_relevant(&self, slot: &Slot) -> bool {
		self.slot_watchlist.read().contains_key(slot)
	}
}

#[cfg(test)]
mod test {
	use crate::oracle::{
		collector::ScpMessageCollector, traits::FileHandler, EnvelopesFileHandler,
		TxSetsFileHandler,
	};
	use frame_support::traits::Len;
	use rand::{
		seq::{IteratorRandom, SliceRandom},
		thread_rng, Rng,
	};
	use stellar_relay::sdk::network::{PUBLIC_NETWORK, TEST_NETWORK};

	#[test]
	fn envelopes_map_len_works() {
		let collector = ScpMessageCollector::new(true, vec![]);

		assert_eq!(collector.envelopes_map_len(), 0);

		let first_slot = 578291;
		let mut env_map = EnvelopesFileHandler::get_map_from_archives(first_slot + 3)
			.expect("should return a map");
		let env_map_len = env_map.len();

		collector.envelopes_map.write().append(&mut env_map);
		assert_eq!(collector.envelopes_map_len(), env_map_len);
	}

	#[test]
	fn network_and_is_public_works() {
		let collector = ScpMessageCollector::new(true, vec![]);
		assert_eq!(&collector.network().get_passphrase(), &PUBLIC_NETWORK.get_passphrase());

		assert!(collector.is_public());

		let collector = ScpMessageCollector::new(false, vec![]);
		assert_eq!(&collector.network().get_passphrase(), &TEST_NETWORK.get_passphrase());
		assert!(!collector.is_public());
	}

	#[test]
	fn watch_slot_works() {
		let mut collector = ScpMessageCollector::new(true, vec![]);

		let slot = 12345;
		collector.watch_slot(slot);

		assert!(collector.slot_watchlist.read().contains_key(&slot));
	}

	#[test]
	fn add_scp_envelope_works() {
		let mut collector = ScpMessageCollector::new(true, vec![]);

		let first_slot = 578291;
		let mut env_map =
			EnvelopesFileHandler::get_map_from_archives(first_slot).expect("should return a map");

		let mut env_map_keys = env_map.keys();
		let slot = 1234;

		let x = env_map_keys.next_back().expect("should return a slot");
		let value = env_map.get(x).expect("should return a vec of scp envelopes");
		let one_scp_env = value[0].clone();
		collector.add_scp_envelope(slot, one_scp_env.clone());

		assert_eq!(collector.envelopes_map_len(), 1);
		assert!(collector.envelopes_map.read().contains_key(&slot));

		// let's try to add again.
		let two_scp_env = value[1].clone();
		collector.add_scp_envelope(slot, two_scp_env.clone());
		assert_eq!(collector.envelopes_map_len(), 1); // length shouldn't change, since we're insertin to the same key.

		let collctr_env_map = collector.envelopes_map.read();
		let res = collctr_env_map.get(&slot).expect("should return a vector of scpenvelopes");

		assert_eq!(res.len(), 2);
		assert_eq!(&res[0], &one_scp_env);
		assert_eq!(&res[1], &two_scp_env);
	}

	#[test]
	fn add_txset_works() {
		let mut collector = ScpMessageCollector::new(false, vec![]);

		let slot = 42867088;
		let txsets_map =
			TxSetsFileHandler::get_map_from_archives(slot).expect("should return a map");
		let value = txsets_map.get(&slot).expect("should return a transaction set");

		collector.add_txset(slot, value.clone());

		assert!(collector.txset_map.read().contains_key(&slot));
	}

	#[test]
	fn insert_to_pending_list_works() {
		let mut collector = ScpMessageCollector::new(false, vec![]);

		collector.save_txset_hash_and_slot([0; 32], 0);
		collector.save_txset_hash_and_slot([1; 32], 1);

		collector.insert_to_pending_list(&[1; 32]);
		assert!(collector.slot_pendinglist.read().contains(&1));

		// trying to insert a non-existing txset_hash
		collector.insert_to_pending_list(&[2; 32]);
		assert!(!collector.slot_pendinglist.read().contains(&2));
	}

	#[test]
	fn set_last_slot_index_works() {
		let mut collector = ScpMessageCollector::new(true, vec![]);
		{
			let mut idx = collector.last_slot_index.write();
			*idx = 10;
		}
		{
			collector.set_last_slot_index(9);
			// there should be no change.
			let res = collector.last_slot_index.read();
			assert_eq!(*res, 10);
		}
		{
			collector.set_last_slot_index(15);
			let res = collector.last_slot_index.read();
			assert_eq!(*res, 15);
		}
	}

	#[test]
	fn remove_data_works() {
		let mut collector = ScpMessageCollector::new(false, vec![]);

		let env_slot = 578291;
		let mut env_map =
			EnvelopesFileHandler::get_map_from_archives(env_slot).expect("should return a map");

		let txset_slot = 42867088;
		let txsets_map =
			TxSetsFileHandler::get_map_from_archives(txset_slot).expect("should return a map");

		collector.watch_slot(env_slot);
		collector.envelopes_map.write().append(&mut env_map);

		let txset = txsets_map.get(&txset_slot).expect("should return a tx set");
		collector.txset_map.write().insert(env_slot, txset.clone());

		collector.slot_pendinglist.write().push(env_slot);

		assert!(
			collector.envelopes_map.read().contains_key(&env_slot) &&
				collector.txset_map.read().contains_key(&env_slot) &&
				collector.slot_watchlist.read().contains_key(&env_slot) &&
				collector.slot_pendinglist.read().contains(&env_slot)
		);

		collector.remove_data(&env_slot);
		assert!(
			!collector.envelopes_map.read().contains_key(&env_slot) &&
				!collector.txset_map.read().contains_key(&env_slot) &&
				!collector.slot_watchlist.read().contains_key(&env_slot) &&
				!collector.slot_pendinglist.read().contains(&env_slot)
		);
	}

	#[test]
	fn is_txset_new_works() {
		let mut collector = ScpMessageCollector::new(false, vec![]);

		let txset_slot = 42867088;
		let mut txsets_map =
			TxSetsFileHandler::get_map_from_archives(txset_slot).expect("should return a map");
		collector.txset_map.write().append(&mut txsets_map);

		collector.txset_and_slot_map.write().insert([0; 32], 0);

		// these didn't exist yet.
		assert!(collector.is_txset_new(&[1; 32], &5));

		// the hash exists
		assert!(!collector.is_txset_new(&[0; 32], &6));
		// the slot exists
		assert!(!collector.is_txset_new(&[3; 32], &txset_slot));
	}

	#[test]
	fn is_slot_relevant_works() {
		let mut collector = ScpMessageCollector::new(false, vec![]);

		collector.slot_watchlist.write().insert(123, ());
		collector.slot_watchlist.write().insert(456, ());

		assert!(collector.is_slot_relevant(&456));
		assert!(collector.is_slot_relevant(&123));
		assert!(!collector.is_slot_relevant(&789));
	}
}
