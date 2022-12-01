#![allow(non_snake_case)]

use std::collections::{BTreeMap, HashMap};

use stellar_relay_lib::sdk::types::{Hash, ScpEnvelope, TransactionSet, Uint64};

pub type Slot = Uint64;
pub type TxHash = Hash;
pub type TxSetHash = Hash;
pub type Filename = String;

pub type SerializedData = Vec<u8>;

/// For easy writing to file. BTreeMap to preserve order of the slots.
pub(crate) type SlotEncodedMap = BTreeMap<Slot, SerializedData>;

/// Sometimes not enough `StellarMessage::ScpMessage(...)` are sent per slot;
/// or that the `Stellar:message::TxSet(...)` took too long to arrive (may not even arrive at all)
/// So I've kept both of them separate: the `EnvelopesMap` and the `TxSetMap`
pub(crate) type EnvelopesMap = BTreeMap<Slot, Vec<ScpEnvelope>>;
pub(crate) type TxSetMap = BTreeMap<Slot, TransactionSet>;

pub(crate) type SlotList = BTreeMap<Slot, ()>;

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
		self.hash_slot.insert(hash.clone(), slot);
		self.slot_hash.insert(slot, hash);
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
