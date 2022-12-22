use std::sync::Arc;

use parking_lot::{lock_api::RwLockReadGuard, RawRwLock, RwLock};

use stellar_relay_lib::sdk::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::{ScpEnvelope, TransactionSet},
};

use crate::oracle::types::{EnvelopesMap, LifoMap, Slot, TxSetHash, TxSetHashAndSlotMap, TxSetMap};

/// Collects all ScpMessages and the TxSets.
pub struct ScpMessageCollector {
	/// holds the mapping of the Slot Number(key) and the ScpEnvelopes(value)
	envelopes_map: Arc<RwLock<EnvelopesMap>>,

	/// holds the mapping of the Slot Number(key) and the TransactionSet(value)
	txset_map: Arc<RwLock<TxSetMap>>,

	/// Mapping between the txset's hash and its corresponding slot.
	/// An entry is removed when a `TransactionSet` is found.
	txset_and_slot_map: Arc<RwLock<TxSetHashAndSlotMap>>,

	last_slot_index: Arc<RwLock<u64>>,

	public_network: bool,
}

impl ScpMessageCollector {
	pub(crate) fn new(public_network: bool) -> Self {
		ScpMessageCollector {
			envelopes_map: Default::default(),
			txset_map: Default::default(),
			txset_and_slot_map: Arc::new(Default::default()),
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

	pub(super) fn txset_map_clone(&self) -> Arc<RwLock<TxSetMap>> {
		self.txset_map.clone()
	}

	pub(super) fn get_txset_hash(&self, slot: &Slot) -> Option<TxSetHash> {
		self.txset_and_slot_map.read().get_txset_hash(slot).cloned()
	}

	pub(crate) fn last_slot_index(&self) -> RwLockReadGuard<'_, RawRwLock, u64> {
		self.last_slot_index.read()
	}
}

// insert/add/save functions
impl ScpMessageCollector {
	pub(super) fn add_scp_envelope(&self, slot: Slot, scp_envelope: ScpEnvelope) {
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
	}

	pub(super) fn add_txset(&self, txset_hash: &TxSetHash, tx_set: TransactionSet) {
		let slot = {
			let mut map_write = self.txset_and_slot_map.write();
			map_write.remove_by_txset_hash(txset_hash).map(|slot| {
				tracing::info!("saved txset_hash for slot: {:?}", slot);
				self.txset_map.write().set_with_key(slot, tx_set);
				slot
			})
		};

		if slot.is_none() {
			tracing::warn!("WARNING! tx_set_hash: {:?} has no slot.", txset_hash);
		}
	}

	pub(super) fn save_txset_hash_and_slot(&self, txset_hash: TxSetHash, slot: Slot) {
		// save the mapping of the hash of the txset and the slot.
		let mut m = self.txset_and_slot_map.write();
		m.insert(txset_hash, slot);
	}

	pub(super) fn set_last_slot_index(&self, slot: Slot) {
		let mut last_slot_index = self.last_slot_index.write();
		if slot > *last_slot_index {
			*last_slot_index = slot;
		}
	}
}

// delete/remove functions
impl ScpMessageCollector {
	/// Clear out data related to this slot.
	pub(crate) fn remove_data(&self, slot: &Slot) {
		self.envelopes_map.write().remove_with_key(slot);
		self.txset_map.write().remove_with_key(slot);
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
}

#[cfg(test)]
mod test {
	use stellar_relay_lib::sdk::network::{PUBLIC_NETWORK, TEST_NETWORK};

	use crate::oracle::{
		collector::ScpMessageCollector, traits::FileHandler, types::LifoMap, EnvelopesFileHandler,
		TxSetsFileHandler,
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
	fn add_scp_envelope_works() {
		let collector = ScpMessageCollector::new(true);

		let first_slot = 578291;
		let env_map =
			EnvelopesFileHandler::get_map_from_archives(first_slot).expect("should return a map");

		let _slot = 1234;

		let (slot, value) = env_map.get(0).expect("should return a tuple");
		let one_scp_env = value[0].clone();
		collector.add_scp_envelope(*slot, one_scp_env.clone());

		assert_eq!(collector.envelopes_map_len(), 1);
		assert!(collector.envelopes_map.read().contains_key(slot));

		// let's try to add again.
		let two_scp_env = value[1].clone();
		collector.add_scp_envelope(*slot, two_scp_env.clone());
		assert_eq!(collector.envelopes_map_len(), 1); // length shouldn't change, since we're insertin to the same key.

		let collctr_env_map = collector.envelopes_map.read();
		let res = collctr_env_map
			.get_with_key(slot)
			.expect("should return a vector of scpenvelopes");

		assert_eq!(res.len(), 2);
		assert_eq!(&res[0], &one_scp_env);
		assert_eq!(&res[1], &two_scp_env);
	}

	#[test]
	fn add_txset_works() {
		let collector = ScpMessageCollector::new(false);

		let slot = 42867088;
		let dummy_hash = [0; 32];
		let txsets_map =
			TxSetsFileHandler::get_map_from_archives(slot).expect("should return a map");
		let value = txsets_map.get_with_key(&slot).expect("should return a transaction set");

		collector.save_txset_hash_and_slot(dummy_hash, slot);

		collector.add_txset(&dummy_hash, value.clone());

		assert!(collector.txset_map.read().contains_key(&slot));
	}

	#[test]
	fn set_last_slot_index_works() {
		let collector = ScpMessageCollector::new(true);
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
		let collector = ScpMessageCollector::new(false);

		let env_slot = 578391;
		let mut env_map =
			EnvelopesFileHandler::get_map_from_archives(env_slot).expect("should return a map");

		let txset_slot = 42867088;
		let txsets_map =
			TxSetsFileHandler::get_map_from_archives(txset_slot).expect("should return a map");

		// collector.watch_slot(env_slot);
		collector.envelopes_map.write().append(&mut env_map);

		let txset = txsets_map.get_with_key(&txset_slot).expect("should return a tx set");
		collector.txset_map.write().set_with_key(env_slot, txset.clone());

		assert!(collector.envelopes_map.read().contains_key(&env_slot));
		assert!(collector.txset_map.read().contains_key(&env_slot));
		// assert!(collector.slot_watchlist.read().contains_key(&env_slot));

		collector.remove_data(&env_slot);
		assert!(!collector.envelopes_map.read().contains_key(&env_slot));
		assert!(!collector.txset_map.read().contains_key(&env_slot));
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
}
