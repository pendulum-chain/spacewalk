use std::sync::Arc;

use parking_lot::{lock_api::RwLockReadGuard, RawRwLock, RwLock};

use stellar_relay_lib::sdk::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::{ScpEnvelope, TransactionSet},
};

use crate::oracle::types::{EnvelopesMap, Slot, TxSetHash, TxSetHashAndSlotMap, TxSetMap};

/// Collects all ScpMessages and the TxSets.
pub struct ScpMessageCollector {
	/// holds the mapping of the Slot Number(key) and the ScpEnvelopes(value)
	envelopes_map: Arc<RwLock<EnvelopesMap>>,

	/// holds the mapping of the Slot Number(key) and the TransactionSet(value)
	txset_map: Arc<RwLock<TxSetMap>>,

	/// Mapping between the txset's hash and its corresponding slot.
	/// An entry is removed when a `TransactionSet` is found.
	txset_and_slot_map: Arc<RwLock<TxSetHashAndSlotMap>>,

	/// The last slot with an SCPEnvelope
	last_slot_index: u64,

	public_network: bool,

	// A (possibly empty) list of URLs to use to fetch the stellar history archive entries.
	stellar_history_archive_urls: Vec<String>,
}

impl ScpMessageCollector {
	pub(crate) fn new(public_network: bool, stellar_history_archive_urls: Vec<String>) -> Self {
		ScpMessageCollector {
			envelopes_map: Default::default(),
			txset_map: Default::default(),
			txset_and_slot_map: Arc::new(Default::default()),
			last_slot_index: 0,
			public_network,
			stellar_history_archive_urls,
		}
	}

	pub(crate) fn new_with_size_limit(
		public_network: bool,
		stellar_history_archive_urls: Vec<String>,
		size_limit: usize,
	) -> Self {
		ScpMessageCollector {
			envelopes_map: Arc::new(RwLock::new(EnvelopesMap::new().with_limit(size_limit))),
			txset_map: Arc::new(RwLock::new(TxSetMap::new().with_limit(size_limit))),
			txset_and_slot_map: Arc::new(Default::default()),
			last_slot_index: 0,
			public_network,
			stellar_history_archive_urls,
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

	pub fn stellar_history_archive_urls(&self) -> Vec<String> {
		self.stellar_history_archive_urls.clone()
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

	pub(crate) fn last_slot_index(&self) -> u64 {
		self.last_slot_index
	}
}

// insert/add/save functions
impl ScpMessageCollector {
	pub(super) fn add_scp_envelope(&self, slot: Slot, scp_envelope: ScpEnvelope) {
		// insert/add the externalized message to map.
		let mut envelopes_map = self.envelopes_map.write();

		if let Some(value) = envelopes_map.get(&slot) {
			let mut value = value.clone();
			value.push(scp_envelope);
			envelopes_map.insert(slot, value);
		} else {
			tracing::debug!("Collecting SCPEnvelopes for slot {slot}: success");
			tracing::trace!(
				"Collecting SCPEnvelopes for slot {slot}: the scp envelope: {scp_envelope:?}"
			);
			envelopes_map.insert(slot, vec![scp_envelope]);
		}
	}

	pub(super) fn add_txset(&self, txset_hash: &TxSetHash, tx_set: TransactionSet) {
		let hash_str = hex::encode(&txset_hash);

		let slot = {
			let mut map_write = self.txset_and_slot_map.write();
			map_write.remove_by_txset_hash(txset_hash).map(|slot| {
				tracing::debug!("Collecting TxSet for slot {slot}: txset saved.");
				tracing::trace!("Collecting TxSet for slot {slot}: {tx_set:?}");
				self.txset_map.write().insert(slot, tx_set);
				slot
			})
		};

		if slot.is_none() {
			tracing::warn!("Collecting TxSet for slot: tx_set_hash: {hash_str} has no slot.");
		}
	}

	pub(super) fn save_txset_hash_and_slot(&self, txset_hash: TxSetHash, slot: Slot) {
		// save the mapping of the hash of the txset and the slot.
		let mut m = self.txset_and_slot_map.write();
		tracing::debug!("Collecting TxSet for slot {slot}: saving a map of txset_hash...");
		let hash = hex::encode(&txset_hash);
		tracing::trace!("Collecting TxSet for slot {slot}: the txset_hash: {hash}");
		m.insert(txset_hash, slot);
	}

	pub(super) fn set_last_slot_index(&mut self, slot: Slot) {
		if slot > self.last_slot_index {
			self.last_slot_index = slot;
		}
	}
}

// delete/remove functions
impl ScpMessageCollector {
	/// Clear out data related to this slot.
	pub(crate) fn remove_data(&self, slot: &Slot) {
		self.envelopes_map.write().remove(slot);
		self.txset_map.write().remove(slot);
	}
}

// checking and other implementations
impl ScpMessageCollector {
	/// checks whether the txset hash and slot tandem is already recorded/noted/flagged.
	pub(super) fn is_txset_new(&self, txset_hash: &TxSetHash, slot: &Slot) -> bool {
		self.txset_and_slot_map.read().get_slot(txset_hash).is_none() &&
            // also check whether this is a delayed message
            !self.txset_map.read().contains(slot)
	}
}

#[cfg(test)]
mod test {
	use stellar_relay_lib::sdk::network::{PUBLIC_NETWORK, TEST_NETWORK};

	use crate::oracle::{
		collector::ScpMessageCollector, get_test_stellar_relay_config, traits::FileHandler,
		EnvelopesFileHandler, TxSetsFileHandler,
	};

	fn stellar_history_archive_urls() -> Vec<String> {
		get_test_stellar_relay_config(true).stellar_history_archive_urls()
	}

	#[test]
	fn envelopes_map_len_works() {
		let collector = ScpMessageCollector::new(true, stellar_history_archive_urls());

		assert_eq!(collector.envelopes_map_len(), 0);

		let first_slot = 578291;
		let env_map = EnvelopesFileHandler::get_map_from_archives(first_slot + 3)
			.expect("should return a map");
		let env_map_len = env_map.len();

		collector.envelopes_map.write().append(env_map);
		assert_eq!(collector.envelopes_map_len(), env_map_len);
	}

	#[test]
	fn network_and_is_public_works() {
		let collector = ScpMessageCollector::new(true, stellar_history_archive_urls());
		assert_eq!(&collector.network().get_passphrase(), &PUBLIC_NETWORK.get_passphrase());

		assert!(collector.is_public());

		let collector = ScpMessageCollector::new(false, stellar_history_archive_urls());
		assert_eq!(&collector.network().get_passphrase(), &TEST_NETWORK.get_passphrase());
		assert!(!collector.is_public());
	}

	#[test]
	fn add_scp_envelope_works() {
		let collector = ScpMessageCollector::new(true, stellar_history_archive_urls());

		let first_slot = 578291;
		let env_map =
			EnvelopesFileHandler::get_map_from_archives(first_slot).expect("should return a map");

		let (slot, value) = env_map.first().expect("should return a tuple");
		let one_scp_env = value[0].clone();
		collector.add_scp_envelope(*slot, one_scp_env.clone());

		assert_eq!(collector.envelopes_map_len(), 1);
		assert!(collector.envelopes_map.read().contains(slot));

		// let's try to add again.
		let two_scp_env = value[1].clone();
		collector.add_scp_envelope(*slot, two_scp_env.clone());

		// length shouldn't change, since we're inserting to the same key.
		assert_eq!(collector.envelopes_map_len(), 1);

		let collctr_env_map = collector.envelopes_map.read();
		let res = collctr_env_map.get(slot).expect("should return a vector of scpenvelopes");

		assert_eq!(res.len(), 2);
		assert_eq!(&res[0], &one_scp_env);
		assert_eq!(&res[1], &two_scp_env);
	}

	#[test]
	fn add_txset_works() {
		let collector = ScpMessageCollector::new(false, stellar_history_archive_urls());

		let slot = 42867088;
		let dummy_hash = [0; 32];
		let txsets_map =
			TxSetsFileHandler::get_map_from_archives(slot).expect("should return a map");
		let value = txsets_map.get(&slot).expect("should return a transaction set");

		collector.save_txset_hash_and_slot(dummy_hash, slot);

		collector.add_txset(&dummy_hash, value.clone());

		assert!(collector.txset_map.read().contains(&slot));
	}

	#[test]
	fn set_last_slot_index_works() {
		let mut collector = ScpMessageCollector::new(true, stellar_history_archive_urls());
		collector.last_slot_index = 10;

		collector.set_last_slot_index(9);
		// there should be no change.
		let res = collector.last_slot_index;
		assert_eq!(res, 10);

		collector.set_last_slot_index(15);
		let res = collector.last_slot_index;
		assert_eq!(res, 15);
	}

	#[test]
	fn remove_data_works() {
		let collector = ScpMessageCollector::new(false, stellar_history_archive_urls());

		let env_slot = 578391;
		let env_map =
			EnvelopesFileHandler::get_map_from_archives(env_slot).expect("should return a map");

		let txset_slot = 42867088;
		let txsets_map =
			TxSetsFileHandler::get_map_from_archives(txset_slot).expect("should return a map");

		// collector.watch_slot(env_slot);
		collector.envelopes_map.write().append(env_map);

		let txset = txsets_map.get(&txset_slot).expect("should return a tx set");
		collector.txset_map.write().insert(env_slot, txset.clone());

		assert!(collector.envelopes_map.read().contains(&env_slot));
		assert!(collector.txset_map.read().contains(&env_slot));
		// assert!(collector.slot_watchlist.read().contains_key(&env_slot));

		collector.remove_data(&env_slot);
		assert!(!collector.envelopes_map.read().contains(&env_slot));
		assert!(!collector.txset_map.read().contains(&env_slot));
	}

	#[test]
	fn is_txset_new_works() {
		let collector = ScpMessageCollector::new(false, stellar_history_archive_urls());

		let txset_slot = 42867088;
		let txsets_map =
			TxSetsFileHandler::get_map_from_archives(txset_slot).expect("should return a map");

		let txsets_size = txsets_map.len();
		println!("txsets size: {}", txsets_size);

		collector.txset_map.write().append(txsets_map);

		collector.txset_and_slot_map.write().insert([0; 32], 0);

		// these didn't exist yet.
		assert!(collector.is_txset_new(&[1; 32], &5));

		// the hash exists
		assert!(!collector.is_txset_new(&[0; 32], &6));
		// the slot exists
		assert!(!collector.is_txset_new(&[3; 32], &txset_slot));
	}
}
