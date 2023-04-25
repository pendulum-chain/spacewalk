#![allow(non_snake_case)]

use std::{
	collections::{hash_map::Iter, BTreeMap, HashMap, VecDeque},
	fmt::Debug,
};

use itertools::Itertools;
use tokio::sync::mpsc;

use stellar_relay_lib::sdk::types::{Hash, ScpEnvelope, StellarMessage, TransactionSet, Uint64};

pub type Slot = Uint64;
pub type TxHash = Hash;
pub type TxSetHash = Hash;
pub type Filename = String;

pub type SerializedData = Vec<u8>;

pub type StellarMessageSender = mpsc::Sender<StellarMessage>;

/// For easy writing to file. BTreeMap to preserve order of the slots.
pub(crate) type SlotEncodedMap = BTreeMap<Slot, SerializedData>;

/// Sometimes not enough `StellarMessage::ScpMessage(...)` are sent per slot;
/// or that the `Stellar:message::TxSet(...)` took too long to arrive (may not even arrive at all)
/// So I've kept both of them separate: the `EnvelopesMap` and the `TxSetMap`
pub(crate) type EnvelopesMap = LimitedFifoMap<Slot, Vec<ScpEnvelope>>;
pub(crate) type TxSetMap = LimitedFifoMap<Slot, TransactionSet>;

pub(crate) type SlotList = BTreeMap<Slot, ()>;

const FIFOMAP_MIN_LIMIT: usize = 200;

#[derive(Debug, Clone)]
pub struct LimitedFifoMap<K, T> {
	limit: usize,
	queue: VecDeque<(K, T)>,
}

impl<K, T> LimitedFifoMap<K, T>
where
	K: Debug + PartialEq,
{
	pub fn new() -> Self {
		LimitedFifoMap { limit: FIFOMAP_MIN_LIMIT, queue: VecDeque::new() }
	}

	pub fn with_limit(mut self, limit: usize) -> Self {
		if limit < FIFOMAP_MIN_LIMIT {
			self.limit = FIFOMAP_MIN_LIMIT
		} else {
			self.limit = limit;
		}

		self
	}

	pub fn limit(&self) -> usize {
		self.limit
	}

	pub fn len(&self) -> usize {
		self.queue.len()
	}

	pub fn contains(&self, key: &K) -> bool {
		self.queue.iter().any(|(k, _)| k == key)
	}

	pub fn get(&self, key: &K) -> Option<&T> {
		self.queue.iter().find(|(k, _)| k == key).map(|(_, v)| v)
	}

	pub fn first(&self) -> Option<&(K, T)> {
		self.queue.get(0)
	}

	pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, (K, T)> {
		self.queue.iter()
	}

	pub fn remove(&mut self, key: &K) -> Option<T> {
		let (index, _) = self.queue.iter().find_position(|(k, _)| k == key)?;
		self.queue.remove(index).map(|(_, v)| v)
	}

	pub fn insert(&mut self, key: K, value: T) -> Option<T> {
		let old_value = self.remove(&key);

		// remove the oldest entry if the queue reached its limit
		if self.queue.len() == self.limit {
			if let Some(oldest_entry) = self.queue.pop_front() {
				println!("removing old entry with key: {:?}", oldest_entry.0);
				tracing::debug!("removing old entry with key: {:?}", oldest_entry.0);
			}
		}

		self.queue.push_back((key, value));

		old_value
	}

	/// Consumes the other and returns the excess of it. The limit will be based on self.
	pub fn append(&mut self, other: Self) -> VecDeque<(K, T)> {
		// check the remaining size available for this map.
		let allowable_size = self.limit - self.len();

		let mut other_queue = other.queue;
		let last_partition = other_queue.split_off(allowable_size);

		self.queue.append(&mut other_queue);

		last_partition
	}
}

impl<K: PartialEq + Debug, T> Default for LimitedFifoMap<K, T> {
	fn default() -> Self {
		LimitedFifoMap::new()
	}
}

/// The slot is not found in the `StellarMessage::TxSet(...)`, therefore this map
/// serves as a holder of the slot when we hash the txset.
#[derive(Clone)]
pub struct TxSetHashAndSlotMap {
	hash_slot: HashMap<TxSetHash, Slot>,
	slot_hash: HashMap<Slot, TxSetHash>,
}

impl Default for TxSetHashAndSlotMap {
	fn default() -> Self {
		TxSetHashAndSlotMap::new()
	}
}

impl TxSetHashAndSlotMap {
	pub fn new() -> Self {
		TxSetHashAndSlotMap { hash_slot: Default::default(), slot_hash: Default::default() }
	}

	pub fn get_slot(&self, hash: &TxSetHash) -> Option<&Slot> {
		self.hash_slot.get(hash)
	}

	pub fn get_txset_hash(&self, slot: &Slot) -> Option<&TxSetHash> {
		self.slot_hash.get(slot)
	}

	pub fn remove_by_slot(&mut self, slot: &Slot) -> Option<TxSetHash> {
		let hash = self.slot_hash.remove(slot)?;
		self.hash_slot.remove(&hash)?;

		Some(hash)
	}

	pub fn remove_by_txset_hash(&mut self, txset_hash: &TxSetHash) -> Option<Slot> {
		let slot = self.hash_slot.remove(txset_hash)?;
		self.slot_hash.remove(&slot)?;
		Some(slot)
	}

	pub fn insert(&mut self, hash: TxSetHash, slot: Slot) {
		self.hash_slot.insert(hash, slot);
		self.slot_hash.insert(slot, hash);
	}
}

#[cfg(test)]
mod test {
	use crate::oracle::types::{LimitedFifoMap, TxSetHashAndSlotMap, FIFOMAP_MIN_LIMIT};
	use std::convert::TryFrom;

	#[test]
	fn test_LimitedFifoMap() {
		let sample = LimitedFifoMap::<u32, char>::new();

		// --------- test limit ---------
		assert_eq!(sample.limit(), FIFOMAP_MIN_LIMIT);

		// change limit success
		let expected_limit = 500;
		let new_sample = sample.with_limit(expected_limit);
		assert_eq!(new_sample.limit(), expected_limit);

		// change limit is less than minimum
		let expected_limit = 199;
		let mut sample_map = new_sample.with_limit(expected_limit);
		assert_ne!(sample_map.limit(), expected_limit);
		assert_eq!(sample_map.limit(), FIFOMAP_MIN_LIMIT);

		// --------- test insert and len methods ---------
		let fill_size = sample_map.limit();

		for x in 0..fill_size {
			let key = u32::try_from(x).expect("should return ok");
			let value = char::from_u32(key).unwrap_or('x');

			println!("insert: {} {}", key, value);
			assert_eq!(sample_map.insert(key, value), None);
			println!("value of x: {} and len {}", x, sample_map.len());
			assert_eq!(sample_map.len(), x + 1);
		}

		// insert an existing entry
		let new_value = 'a';
		let key_10 = 10;
		let old_value = sample_map.insert(key_10, new_value);
		assert_eq!(old_value, char::from_u32(key_10));

		// insert a new entry, removing the old one.
		let key_300 = 300;
		assert_eq!(sample_map.insert(key_300, new_value), None);

		// --------- test the get method ---------

		// check if the old entry was truly deleted
		assert_eq!(sample_map.get(&0), None);

		// simple get
		assert_eq!(sample_map.get(&key_300), Some(&new_value));
		assert_eq!(sample_map.get(&key_10), Some(&new_value));

		// --------- test contains method ---------
		assert!(!sample_map.contains(&0));
		assert!(sample_map.contains(&1));

		// --------- test remove ---------
		assert_eq!(sample_map.remove(&0), None);
		assert_eq!(sample_map.len(), sample_map.limit());
		assert_eq!(sample_map.remove(&key_10), Some(new_value));
		assert_ne!(sample_map.len(), sample_map.limit());

		let key = 65;
		assert_eq!(sample_map.remove(&key), Some(char::from_u32(key).unwrap_or('x')));
		assert_eq!(sample_map.len(), sample_map.limit() - 2);


		// --------- test append ---------

		// let's populate the 2nd map first
		let limit = 300;
		let mut second_map =  LimitedFifoMap::<u32, char>::new().with_limit(limit);

		let first_len = sample_map.len();
		for x in 0..sample_map.limit() {
			let key = u32::try_from(x + first_len).expect("should return ok");
			let value = char::from_u32(
				u32::try_from(x).expect("should return ok")
			).unwrap_or('x');

			println!("insert: {} {}", key, value);
			assert_eq!(second_map.insert(key, value), None);
			println!("value of x: {} and len {}", x, second_map.len());
		}

		let first_limit = sample_map.limit();
		let second_len = second_map.len();
		let remaining_space = limit - second_len;

		let remaining_first = second_map.append(sample_map.clone());







	}

	#[test]
	fn get_TxSetHashAndSlotMap_tests_works() {
		let mut x = TxSetHashAndSlotMap::new();

		x.insert([0; 32], 0);
		x.insert([1; 32], 1);

		let zero_hash = x.get_txset_hash(&0).expect("should return an array of 32 zeroes inside");
		assert_eq!(*zero_hash, [0; 32]);

		let one_hash = x.get_txset_hash(&1).expect("should return an array of 32 ones inside");
		assert_eq!(*one_hash, [1; 32]);

		let zero_slot = x.get_slot(&[0; 32]).expect("should return a zero slot");
		assert_eq!(*zero_slot, 0);

		let one_slot = x.get_slot(&[1; 32]).expect("should return the one slot");
		assert_eq!(*one_slot, 1);
	}

	#[test]
	fn remove_TxSetHashAndSlotMap_tests_works() {
		let mut x = TxSetHashAndSlotMap::new();

		x.insert([0; 32], 0);
		x.insert([1; 32], 1);
		x.insert([2; 32], 2);

		x.remove_by_slot(&1);
		assert_eq!(x.get_txset_hash(&1), None);
		assert_eq!(x.get_slot(&[1; 32]), None);

		x.remove_by_txset_hash(&[2; 32]);
		assert_eq!(x.get_slot(&[2; 32]), None);
		assert_eq!(x.get_txset_hash(&2), None);
	}
}
