#![allow(non_snake_case)]

use std::{
	clone::Clone,
	cmp::Eq,
	collections::{BTreeMap, HashMap, VecDeque},
	fmt::Debug,
};

use itertools::Itertools;
use tokio::sync::mpsc;

use crate::oracle::constants::DEFAULT_MAX_ITEMS_IN_QUEUE;
use stellar_relay_lib::sdk::types::{Hash, ScpEnvelope, StellarMessage, TransactionSet, Uint64};

pub type Slot = Uint64;
pub type TxHash = Hash;
pub type TxSetHash = Hash;
pub type Filename = String;

pub type SerializedData = Vec<u8>;

pub type StellarMessageSender = mpsc::Sender<StellarMessage>;

/// The slot is not found in the `StellarMessage::TxSet(...)`, therefore this map
/// serves as a holder of the slot when we hash the txset.
pub type TxSetHashAndSlotMap = DoubleSidedHashMap<TxSetHash, Slot>;

/// For easy writing to file. BTreeMap to preserve order of the slots.
pub(crate) type SlotEncodedMap = BTreeMap<Slot, SerializedData>;

/// Sometimes not enough `StellarMessage::ScpMessage(...)` are sent per slot;
/// or that the `Stellar:message::TxSet(...)` took too long to arrive (may not even arrive at all)
/// So I've kept both of them separate: the `EnvelopesMap` and the `TxSetMap`
pub(crate) type EnvelopesMap = LimitedFifoMap<Slot, Vec<ScpEnvelope>>;
pub(crate) type TxSetMap = LimitedFifoMap<Slot, TransactionSet>;

pub(crate) type SlotList = BTreeMap<Slot, ()>;

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
		LimitedFifoMap { limit: DEFAULT_MAX_ITEMS_IN_QUEUE, queue: VecDeque::new() }
	}

	pub fn with_limit(mut self, limit: usize) -> Self {
		// cannot set a number smaller than the default, because if the limit is too small,
		// restoring SPC messages from an archive might have issues
		// since the archived data is inserted in large batches.
		if limit < DEFAULT_MAX_ITEMS_IN_QUEUE {
			self.limit = DEFAULT_MAX_ITEMS_IN_QUEUE
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
				tracing::trace!(
					"LimitedFifoMap: removing old entry with key: {:?}",
					oldest_entry.0
				);
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
		let other_queue_len = other_queue.len();

		if other_queue_len > allowable_size {
			let split_index = other_queue_len - allowable_size;
			// split off the 'other' map, since it's too big to append all of its elements
			let mut last_partition = other_queue.split_off(split_index);

			self.queue.append(&mut last_partition);

			other_queue
		} else {
			self.queue.append(&mut other_queue);
			VecDeque::new()
		}
	}
}

impl<K: PartialEq + Debug, T> Default for LimitedFifoMap<K, T> {
	fn default() -> Self {
		LimitedFifoMap::new()
	}
}

#[derive(Clone)]
pub struct DoubleSidedHashMap<T1, T2> {
	t1_to_t2_map: HashMap<T1, T2>,
	t2_to_t1_map: HashMap<T2, T1>,
}

impl<T1, T2> Default for DoubleSidedHashMap<T1, T2>
where
	T1: Clone + Eq + std::hash::Hash,
	T2: Clone + Eq + std::hash::Hash,
{
	fn default() -> Self {
		DoubleSidedHashMap::new()
	}
}

impl<T1, T2> DoubleSidedHashMap<T1, T2>
where
	T1: Clone + Eq + std::hash::Hash,
	T2: Clone + Eq + std::hash::Hash,
{
	pub fn new() -> Self {
		DoubleSidedHashMap { t1_to_t2_map: Default::default(), t2_to_t1_map: Default::default() }
	}

	pub fn insert(&mut self, t1: T1, t2: T2) {
		self.t1_to_t2_map.insert(t1.clone(), t2.clone());
		self.t2_to_t1_map.insert(t2, t1);
	}
}

impl DoubleSidedHashMap<TxSetHash, Slot> {
	pub fn get_slot_by_txset_hash(&self, hash: &TxSetHash) -> Option<&Slot> {
		self.t1_to_t2_map.get(hash)
	}

	pub fn get_txset_hash_by_slot(&self, slot: &Slot) -> Option<&TxSetHash> {
		self.t2_to_t1_map.get(slot)
	}

	pub fn remove_by_slot(&mut self, slot: &Slot) -> Option<TxSetHash> {
		let hash = self.t2_to_t1_map.remove(slot)?;
		self.t1_to_t2_map.remove(&hash)?;

		Some(hash)
	}

	pub fn remove_by_txset_hash(&mut self, txset_hash: &TxSetHash) -> Option<Slot> {
		let slot = self.t1_to_t2_map.remove(txset_hash)?;
		self.t2_to_t1_map.remove(&slot)?;
		Some(slot)
	}
}

#[cfg(test)]
mod test {
	use crate::oracle::types::{LimitedFifoMap, TxSetHashAndSlotMap, DEFAULT_MAX_ITEMS_IN_QUEUE};
	use std::convert::TryFrom;

	#[test]
	fn test_LimitedFifoMap() {
		let sample = LimitedFifoMap::<u32, char>::new();

		// --------- test limit ---------
		assert_eq!(sample.limit(), DEFAULT_MAX_ITEMS_IN_QUEUE);

		// change limit success
		let expected_limit = 500;
		let new_sample = sample.with_limit(expected_limit);
		assert_eq!(new_sample.limit(), expected_limit);

		// change limit is less than minimum
		let expected_limit = 199;
		let mut sample_map = new_sample.with_limit(expected_limit);
		assert_ne!(sample_map.limit(), expected_limit);
		assert_eq!(sample_map.limit(), DEFAULT_MAX_ITEMS_IN_QUEUE);

		// --------- test insert and len methods ---------
		let fill_size = sample_map.limit();

		// inserts a char value of the index.
		for x in 0..fill_size {
			let key = u32::try_from(x).expect("should return ok");
			let value = char::from_u32(key).unwrap_or('x');

			assert_eq!(sample_map.insert(key, value), None);
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
		let sample_map_limit = sample_map.limit();
		let mut second_map = LimitedFifoMap::<u32, char>::new().with_limit(limit);

		let first_len = sample_map.len();
		for x in 0..sample_map_limit {
			let key = u32::try_from(x + first_len).expect("should return ok");
			let value = char::from_u32(u32::try_from(x).expect("should return ok")).unwrap_or('x');

			assert_eq!(second_map.insert(key, value), None);
		}

		let second_len = second_map.len();
		let remaining_space = limit - second_len;

		// the remainder should be the old elements of `sample_map`.
		let remainder = second_map.append(sample_map.clone());
		let remainder_len = remainder.len();
		assert_eq!(second_map.len(), limit);
		assert_eq!(remainder_len, sample_map.len() - remaining_space);

		// check that the elements of the remainder list, is the same with the elements
		// of the original sample map.

		let first_remainder = remainder[0];
		let expected = sample_map.queue[0];
		assert_eq!(first_remainder, expected);

		let last_remainder = remainder[remainder_len - 1];
		let expected = sample_map.queue[remainder_len - 1];
		assert_eq!(last_remainder, expected);
	}

	#[test]
	fn get_TxSetHashAndSlotMap_tests_works() {
		let mut x = TxSetHashAndSlotMap::new();

		x.insert([0; 32], 0);
		x.insert([1; 32], 1);

		let zero_hash = x
			.get_txset_hash_by_slot(&0)
			.expect("should return an array of 32 zeroes inside");
		assert_eq!(*zero_hash, [0; 32]);

		let one_hash =
			x.get_txset_hash_by_slot(&1).expect("should return an array of 32 ones inside");
		assert_eq!(*one_hash, [1; 32]);

		let zero_slot = x.get_slot_by_txset_hash(&[0; 32]).expect("should return a zero slot");
		assert_eq!(*zero_slot, 0);

		let one_slot = x.get_slot_by_txset_hash(&[1; 32]).expect("should return the one slot");
		assert_eq!(*one_slot, 1);
	}

	#[test]
	fn remove_TxSetHashAndSlotMap_tests_works() {
		let mut x = TxSetHashAndSlotMap::new();

		x.insert([0; 32], 0);
		x.insert([1; 32], 1);
		x.insert([2; 32], 2);

		x.remove_by_slot(&1);
		assert_eq!(x.get_txset_hash_by_slot(&1), None);
		assert_eq!(x.get_slot_by_txset_hash(&[1; 32]), None);

		x.remove_by_txset_hash(&[2; 32]);
		assert_eq!(x.get_slot_by_txset_hash(&[2; 32]), None);
		assert_eq!(x.get_txset_hash_by_slot(&2), None);
	}
}
