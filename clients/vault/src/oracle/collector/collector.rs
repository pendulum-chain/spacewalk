use itertools::min;
use std::sync::Arc;

use parking_lot::{lock_api::RwLockReadGuard, RawRwLock, RwLock};

use crate::oracle::constants::get_min_externalized_messages;
use stellar_relay_lib::sdk::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::{ScpEnvelope, TransactionSet},
};

use crate::oracle::types::{
	EnvelopesMap, LifoMap, Slot, SlotList, TxSetHash, TxSetHashAndSlotMap, TxSetMap,
};

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
}

impl ScpMessageCollector {
	pub(crate) fn new(public_network: bool) -> Self {
		ScpMessageCollector {
			envelopes_map: Default::default(),
			txset_map: Default::default(),
			txset_and_slot_map: Arc::new(Default::default()),
			slot_pendinglist: Arc::new(Default::default()),
			slot_watchlist: Arc::new(Default::default()),
			last_slot_index: Arc::new(Default::default()),
			public_network,
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

		// check if this slot already exists
		self.is_pending_proof(slot);
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
		{
			let mut envelopes_map = self.envelopes_map.write();

			if let Some(value) = envelopes_map.get_with_key(&slot) {
				let mut value = value.clone();
				value.push(scp_envelope);
				envelopes_map.set_with_key(slot, value);
			} else {
				tracing::debug!("Adding received SCP envelopes for slot {}", slot);
				envelopes_map.set_with_key(slot, vec![scp_envelope]);
			}
		}

		// check if this slot is pending for building proof.
		self.is_pending_proof(slot);
	}

	pub(super) fn add_txset(&mut self, txset_hash: &TxSetHash, tx_set: TransactionSet) {
		let slot = {
			let mut map_write = self.txset_and_slot_map.write();
			map_write.remove_by_txset_hash(txset_hash).map(|slot| {
				self.txset_map.write().set_with_key(slot, tx_set);
				slot
			})
		};

		match slot {
			None => {
				tracing::warn!("WARNING! tx_set_hash: {:?} has no slot.", txset_hash);
			},
			Some(slot) => {
				// check if this slot is pending for building proof.
				self.is_pending_proof(slot);
			},
		}
	}

	pub(super) fn save_txset_hash_and_slot(&mut self, txset_hash: TxSetHash, slot: Slot) {
		// save the mapping of the hash of the txset and the slot.
		let mut m = self.txset_and_slot_map.write();
		m.insert(txset_hash, slot);
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
	pub(crate) fn remove_data(&mut self, slot: &Slot) {
		self.slot_watchlist.write().remove(slot);
		self.envelopes_map.write().remove_with_key(slot);
		self.txset_map.write().remove_with_key(slot);

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

	/// Once a TransactionSet is found, remove that entry in `txset_and_slot` map and then
	/// add to pending list.
	pub(super) fn is_pending_proof(&mut self, slot: Slot) -> bool {
		let env_map_read = self.envelopes_map.read();
		let envs = match env_map_read.get_with_key(&slot) {
			None => return false,
			Some(envs) => envs,
		};

		if envs.len() < get_min_externalized_messages(self.is_public()) {
			return false
		}

		if !self.txset_map.read().contains_key(&slot) {
			return false
		}

		// only slots that are in the watch list should be in the pending list.
		if !self.slot_watchlist.read().contains_key(&slot) {
			return false
		}

		tracing::info!("TESTING TESTING TESTING inserting slot {} to pending list.", slot);
		self.slot_pendinglist.write().push(slot);

		true
	}
}

#[cfg(test)]
mod test {
	use stellar_relay_lib::sdk::{
		network::{PUBLIC_NETWORK, TEST_NETWORK},
		types::TransactionSet,
	};

	use crate::oracle::{
		collector::ScpMessageCollector, constants::get_min_externalized_messages,
		traits::FileHandler, types::LifoMap, EnvelopesFileHandler, TxSetsFileHandler,
	};

	#[test]
	fn envelopes_map_len_works() {
		let collector = ScpMessageCollector::new(true);

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
		let collector = ScpMessageCollector::new(true);
		assert_eq!(&collector.network().get_passphrase(), &PUBLIC_NETWORK.get_passphrase());

		assert!(collector.is_public());

		let collector = ScpMessageCollector::new(false);
		assert_eq!(&collector.network().get_passphrase(), &TEST_NETWORK.get_passphrase());
		assert!(!collector.is_public());
	}

	#[test]
	fn watch_slot_works() {
		let mut collector = ScpMessageCollector::new(true);

		let slot = 12345;
		collector.watch_slot(slot);

		assert!(collector.slot_watchlist.read().contains_key(&slot));
	}

	#[test]
	fn add_scp_envelope_works() {
		let mut collector = ScpMessageCollector::new(true);

		let first_slot = 578291;
		let env_map =
			EnvelopesFileHandler::get_map_from_archives(first_slot).expect("should return a map");

		let slot = 1234;

		let (slot, value) = env_map.get(0).expect("should return a tuple");
		let one_scp_env = value[0].clone();
		collector.add_scp_envelope(*slot, one_scp_env.clone());

		assert_eq!(collector.envelopes_map_len(), 1);
		assert!(collector.envelopes_map.read().contains_key(&slot));

		// let's try to add again.
		let two_scp_env = value[1].clone();
		collector.add_scp_envelope(*slot, two_scp_env.clone());
		assert_eq!(collector.envelopes_map_len(), 1); // length shouldn't change, since we're insertin to the same key.

		let collctr_env_map = collector.envelopes_map.read();
		let res = collctr_env_map
			.get_with_key(&slot)
			.expect("should return a vector of scpenvelopes");

		assert_eq!(res.len(), 2);
		assert_eq!(&res[0], &one_scp_env);
		assert_eq!(&res[1], &two_scp_env);
	}

	#[test]
	fn add_txset_works() {
		let mut collector = ScpMessageCollector::new(false);

		let slot = 42867088;
		let dummy_hash = [0; 32];
		let txsets_map =
			TxSetsFileHandler::get_map_from_archives(slot).expect("should return a map");
		let value = txsets_map.get_with_key(&slot).expect("should return a transaction set");

		collector.save_txset_hash_and_slot(dummy_hash.clone(), slot);

		collector.add_txset(&dummy_hash, value.clone());

		assert!(collector.txset_map.read().contains_key(&slot));
	}

	#[test]
	fn is_pending_proof_works() {
		let is_pub_network = false;
		let min_ext_msgs = get_min_externalized_messages(is_pub_network);

		let dummy_slot_0 = 0;
		let mut collector = ScpMessageCollector::new(is_pub_network);

		// ------------------- prepare scpenvelopes -------------------

		let env_map = {
			let first_slot = 578291;
			EnvelopesFileHandler::get_map_from_archives(first_slot).expect("should return a map")
		};
		{
			let (_, value) = env_map.get(0).expect("should return a tuple");
			let one_scp_env = value[0].clone();

			// let's fill the collector with the minimum # of envelopes
			for i in 0..min_ext_msgs + 1 {
				collector.add_scp_envelope(dummy_slot_0, one_scp_env.clone());
			}
		}

		// ------------------- prepare txset -------------------
		let txsets_map = {
			let slot = 42867088;
			TxSetsFileHandler::get_map_from_archives(slot).expect("should return a map")
		};
		{
			let (_, value) = txsets_map.get(0).expect("should return a transaction set");

			collector.save_txset_hash_and_slot([0; 32], dummy_slot_0);
			collector.add_txset(&[0; 32], value.clone());
		}

		assert!(collector.is_pending_proof(dummy_slot_0));
	}

	#[test]
	fn set_last_slot_index_works() {
		let mut collector = ScpMessageCollector::new(true);
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
		let mut collector = ScpMessageCollector::new(false);

		let env_slot = 578391;
		let mut env_map =
			EnvelopesFileHandler::get_map_from_archives(env_slot).expect("should return a map");

		let txset_slot = 42867088;
		let txsets_map =
			TxSetsFileHandler::get_map_from_archives(txset_slot).expect("should return a map");

		collector.watch_slot(env_slot);
		collector.envelopes_map.write().append(&mut env_map);

		let txset = txsets_map.get_with_key(&txset_slot).expect("should return a tx set");
		collector.txset_map.write().set_with_key(env_slot, txset.clone());

		collector.slot_pendinglist.write().push(env_slot);

		assert!(collector.envelopes_map.read().contains_key(&env_slot));
		assert!(collector.txset_map.read().contains_key(&env_slot));
		assert!(collector.slot_watchlist.read().contains_key(&env_slot));
		assert!(collector.slot_pendinglist.read().contains(&env_slot));

		collector.remove_data(&env_slot);
		assert!(!collector.envelopes_map.read().contains_key(&env_slot));
		assert!(!collector.txset_map.read().contains_key(&env_slot));
		assert!(!collector.slot_watchlist.read().contains_key(&env_slot));
		assert!(!collector.slot_pendinglist.read().contains(&env_slot));
	}

	#[test]
	fn is_txset_new_works() {
		let collector = ScpMessageCollector::new(false);

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
		let collector = ScpMessageCollector::new(false);

		collector.slot_watchlist.write().insert(123, ());
		collector.slot_watchlist.write().insert(456, ());

		assert!(collector.is_slot_relevant(&456));
		assert!(collector.is_slot_relevant(&123));
		assert!(!collector.is_slot_relevant(&789));
	}
}
