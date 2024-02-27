#![allow(non_snake_case)]

use crate::oracle::types::TxSetHash;
use std::collections::HashMap;
use wallet::Slot;

/// The slot is not found in the `StellarMessage::TxSet(...)` and
/// `StellarMessage::GeneralizedTxSet(...)`, therefore this map serves as a holder of the slot when
/// we hash the txset.
pub type TxSetHashAndSlotMap = DoubleSidedHashMap<TxSetHash, Slot>;

#[derive(Clone)]
pub struct DoubleSidedHashMap<K, V> {
	k_to_v_map: HashMap<K, V>,
	v_to_k_map: HashMap<V, K>,
}

impl<K, V> Default for DoubleSidedHashMap<K, V>
where
	K: Clone + Eq + std::hash::Hash,
	V: Clone + Eq + std::hash::Hash,
{
	fn default() -> Self {
		DoubleSidedHashMap::new()
	}
}

impl<K, V> DoubleSidedHashMap<K, V>
where
	K: Clone + Eq + std::hash::Hash,
	V: Clone + Eq + std::hash::Hash,
{
	pub fn new() -> Self {
		DoubleSidedHashMap { k_to_v_map: Default::default(), v_to_k_map: Default::default() }
	}

	pub fn insert(&mut self, k: K, v: V) {
		self.k_to_v_map.insert(k.clone(), v.clone());
		self.v_to_k_map.insert(v, k);
	}
}

impl DoubleSidedHashMap<TxSetHash, Slot> {
	pub fn get_slot_by_txset_hash(&self, hash: &TxSetHash) -> Option<&Slot> {
		self.k_to_v_map.get(hash)
	}

	pub fn get_txset_hash_by_slot(&self, slot: &Slot) -> Option<&TxSetHash> {
		self.v_to_k_map.get(slot)
	}

	pub fn remove_by_slot(&mut self, slot: &Slot) -> Option<TxSetHash> {
		let hash = self.v_to_k_map.remove(slot)?;
		self.k_to_v_map.remove(&hash)?;

		Some(hash)
	}

	pub fn remove_by_txset_hash(&mut self, txset_hash: &TxSetHash) -> Option<Slot> {
		let slot = self.k_to_v_map.remove(txset_hash)?;
		self.v_to_k_map.remove(&slot)?;
		Some(slot)
	}
}

#[cfg(test)]
mod test {
	use crate::oracle::types::TxSetHashAndSlotMap;

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
