use crate::oracle::{
	constants::DEFAULT_MAX_ITEMS_IN_QUEUE,
	types::{transaction_set::TxSetBase64Codec, Base64EncodedTxSet, Slot},
	TransactionsArchiveStorage,
};
use itertools::Itertools;
use primitives::stellar::types::{GeneralizedTransactionSet, TransactionSet};
use std::{collections::VecDeque, fmt::Debug};
use stellar_relay_lib::sdk::types::ScpEnvelope;

/// Sometimes not enough `StellarMessage::ScpMessage(...)` are sent per slot;
/// or that the `StellarMessage::TxSet(...)` or `StellarMessage::GeneralizedTxSet(...)`
/// took too long to arrive (may not even arrive at all)
/// So I've kept both of them separate: the `EnvelopesMap` and the `TxSetMap`
//pub(crate) type EnvelopesMap = LimitedFifoMap<Slot, Vec<ScpEnvelope>>;
pub(crate) type EnvelopesMap = LimitedFifoMap<Slot, Vec<ScpEnvelope>>;

/// This map uses the slot as the key and the txset as the value.
/// The txset here can either be the `TransactionSet` or `GeneralizedTransactionSet`
pub(crate) type TxSetMap = LimitedFifoMap<Slot, Base64EncodedTxSet>;

#[derive(Debug, Clone)]
pub struct LimitedFifoMap<K, T> {
	limit: usize,
	queue: VecDeque<(K, T)>,
}

impl<K: PartialEq + Debug, T> Default for LimitedFifoMap<K, T> {
	fn default() -> Self {
		LimitedFifoMap::new()
	}
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

	pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, (K, T)> {
		self.queue.iter()
	}

	pub fn remove(&mut self, key: &K) -> Option<T> {
		let (index, _) = self.queue.iter().find_position(|(k, _)| k == key)?;
		self.queue.remove(index).map(|(_, v)| v)
	}
	pub fn get(&self, key: &K) -> Option<&T> {
		self.queue.iter().find(|(k, _)| k == key).map(|(_, v)| v)
	}

	pub fn first(&self) -> Option<&(K, T)> {
		self.queue.get(0)
	}

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

pub trait AddExt<K, V, T> {
	fn insert(&mut self, key: K, value: V) -> Option<T>;
}

impl<K, T> AddExt<K, T, T> for LimitedFifoMap<K, T>
where
	K: Debug + PartialEq,
{
	fn insert(&mut self, key: K, value: T) -> Option<T> {
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
}

impl<V> AddExt<Slot, V, Base64EncodedTxSet> for TxSetMap
where
	V: TxSetBase64Codec,
{
	fn insert(&mut self, key: Slot, value: V) -> Option<Base64EncodedTxSet> {
		let encoded = value.to_base64_encoded_xdr_string_for_mapping();
		<Self as AddExt<Slot, Base64EncodedTxSet, Base64EncodedTxSet>>::insert(self, key, encoded)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::oracle::types::transaction_set::tests::*;
	use std::convert::TryFrom;
	use stellar_relay_lib::sdk::TransactionEnvelope;

	#[test]
	fn test_LimitedFifoMap_newone() {
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
}
