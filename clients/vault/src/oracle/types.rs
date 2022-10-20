use std::collections::{BTreeMap, HashMap};
use stellar_relay::sdk::TransactionEnvelope;

use crate::oracle::FilterWith;
use stellar_relay::sdk::types::{Hash, ScpEnvelope, TransactionSet, Uint64};

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

pub(crate) type TxHashMap = HashMap<TxHash, Slot>;

/// The slot is not found in the `StellarMessage::TxSet(...)`, therefore this map
/// serves as a holder of the slot when we hash the txset.
pub(crate) type TxSetToSlotMap = HashMap<TxSetHash, Slot>;

/// The FilterWith has to be Send and Sync, as it is sent between channels.
pub(crate) type TxEnvelopeFilter = dyn FilterWith<TransactionEnvelope> + Send + Sync;

/// A map that contains any type that implements FilterWith, explicitly for TransactionEnvelope
pub(crate) type TxFilterMap = HashMap<u32, Box<TxEnvelopeFilter>>;
