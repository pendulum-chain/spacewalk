#![allow(non_snake_case)]

use std::collections::BTreeMap;

use tokio::sync::mpsc;

use stellar_relay_lib::sdk::types::{Hash, StellarMessage, Uint64};

pub type Slot = Uint64;
pub type TxHash = Hash;
pub type TxSetHash = Hash;
pub type Filename = String;

pub type SerializedData = Vec<u8>;

pub type StellarMessageSender = mpsc::Sender<StellarMessage>;

/// For easy writing to file. BTreeMap to preserve order of the slots.
pub(crate) type SlotEncodedMap = BTreeMap<Slot, SerializedData>;

pub(crate) type SlotList = BTreeMap<Slot, ()>;
