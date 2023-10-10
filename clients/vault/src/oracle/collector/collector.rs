use std::{default::Default, sync::Arc};

use parking_lot::{lock_api::RwLockReadGuard, RawRwLock, RwLock};

use runtime::stellar::types::GeneralizedTransactionSet;

use stellar_relay_lib::sdk::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::{ScpEnvelope, TransactionSet},
	TransactionSetType,
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
			txset_and_slot_map: Default::default(),
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

	pub(super) fn get_txset_hash_by_slot(&self, slot: &Slot) -> Option<TxSetHash> {
		self.txset_and_slot_map.read().get_txset_hash_by_slot(slot).cloned()
	}

	pub(crate) fn last_slot_index(&self) -> u64 {
		self.last_slot_index
	}
}

pub trait AddTxSet<T> {
	fn add_txset(&self, tx_set: T) -> Result<(), String>;
}

impl AddTxSet<TransactionSet> for ScpMessageCollector {
	fn add_txset(&self, tx_set: TransactionSet) -> Result<(), String> {
		let tx_set = TransactionSetType::TransactionSet(tx_set);
		self.add_txset(tx_set)
	}
}

impl AddTxSet<GeneralizedTransactionSet> for ScpMessageCollector {
	fn add_txset(&self, tx_set: GeneralizedTransactionSet) -> Result<(), String> {
		let tx_set = TransactionSetType::GeneralizedTransactionSet(tx_set);
		self.add_txset(tx_set)
	}
}

impl AddTxSet<TransactionSetType> for ScpMessageCollector {
	fn add_txset(&self, tx_set: TransactionSetType) -> Result<(), String> {
		let hash = tx_set
			.get_tx_set_hash()
			.map_err(|e| format!("failed to get hash of txset:{e:?}"))?;
		let hash_str = hex::encode(&hash);

		let slot = {
			let mut map_write = self.txset_and_slot_map.write();
			map_write.remove_by_txset_hash(&hash).map(|slot| {
				tracing::debug!("Collecting TxSet for slot {slot}: txset saved.");
				tracing::trace!("Collecting TxSet for slot {slot}: {tx_set:?}");
				self.txset_map.write().insert(slot, tx_set);
				slot
			})
		};

		if slot.is_none() {
			tracing::warn!("Collecting TxSet for slot: tx_set_hash: {hash_str} has no slot.");
			return Err(format!("TxSetHash {hash_str} has no slot."))
		}

		Ok(())
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
		self.txset_and_slot_map.read().get_slot_by_txset_hash(txset_hash).is_none() &&
            // also check whether this is a delayed message
            !self.txset_map.read().contains(slot)
	}
}

#[cfg(test)]
mod test {
	use std::{fs::File, io::Read, path::PathBuf};
	use stellar_relay_lib::sdk::{
		network::{PUBLIC_NETWORK, TEST_NETWORK},
		types::{GeneralizedTransactionSet, TransactionSet},
		IntoHash, XdrCodec,
	};

	use crate::oracle::{
		collector::{collector::AddTxSet, ScpMessageCollector},
		get_test_stellar_relay_config,
		traits::FileHandler,
		EnvelopesFileHandler,
	};

	fn open_file(file_name: &str) -> Vec<u8> {
		let mut path = PathBuf::new();
		let path_str = format!("./resources/samples/{file_name}");
		path.push(&path_str);

		let mut file = File::open(path).expect("file should exist");
		let mut bytes: Vec<u8> = vec![];
		let _ = file.read_to_end(&mut bytes).expect("should be able to read until the end");

		bytes
	}

	fn sample_gen_txset_as_vec_u8() -> Vec<u8> {
		open_file("generalized_tx_set")
	}

	fn sample_txset_as_vec_u8() -> Vec<u8> {
		open_file("tx_set")
	}

	pub fn sample_gen_txset() -> GeneralizedTransactionSet {
		let sample = sample_gen_txset_as_vec_u8();

		GeneralizedTransactionSet::from_base64_xdr(sample)
			.expect("should return a generalized transaction set")
	}

	pub fn sample_txset() -> TransactionSet {
		let sample = sample_txset_as_vec_u8();

		TransactionSet::from_base64_xdr(sample).expect("should return a transaction set")
	}

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

		let value = sample_txset();

		let slot = 578391;
		collector.save_txset_hash_and_slot(
			value.clone().into_hash().expect("it should return a hash"),
			slot,
		);

		assert!(collector.add_txset(value).is_ok());

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

	// todo: update this with a new txset sample
	#[test]
	fn remove_data_works() {
		let collector = ScpMessageCollector::new(false, stellar_history_archive_urls());

		let env_slot = 578391;
		let env_map =
			EnvelopesFileHandler::get_map_from_archives(env_slot).expect("should return a map");

		// let txset_slot = 42867088;
		// let txsets_map =
		// 	TxSetsFileHandler::get_map_from_archives(txset_slot).expect("should return a map");

		// collector.watch_slot(env_slot);
		collector.envelopes_map.write().append(env_map);

		// let txset = txsets_map.get(&txset_slot).expect("should return a tx set");
		// collector.txset_map.write().insert(env_slot, txset.clone());

		assert!(collector.envelopes_map.read().contains(&env_slot));
		//assert!(collector.txset_map.read().contains(&env_slot));
		// assert!(collector.slot_watchlist.read().contains_key(&env_slot));

		collector.remove_data(&env_slot);
		assert!(!collector.envelopes_map.read().contains(&env_slot));
		//assert!(!collector.txset_map.read().contains(&env_slot));
	}

	// todo: update this with a new txset sample
	// #[test]
	// fn is_txset_new_works() {
	// 	let collector = ScpMessageCollector::new(false, stellar_history_archive_urls());
	//
	// 	let txset_slot = 42867088;
	// 	let txsets_map =
	// 		TxSetsFileHandler::get_map_from_archives(txset_slot).expect("should return a map");
	//
	// 	let txsets_size = txsets_map.len();
	// 	println!("txsets size: {}", txsets_size);
	//
	// 	collector.txset_map.write().append(txsets_map);
	//
	// 	collector.txset_and_slot_map.write().insert([0; 32], 0);
	//
	// 	// these didn't exist yet.
	// 	assert!(collector.is_txset_new(&[1; 32], &5));
	//
	// 	// the hash exists
	// 	assert!(!collector.is_txset_new(&[0; 32], &6));
	// 	// the slot exists
	// 	assert!(!collector.is_txset_new(&[3; 32], &txset_slot));
	// }
}
