use std::{convert::TryInto, sync::Arc};

use parking_lot::RwLock;
use tokio::sync::{mpsc, mpsc::error::SendError};

use stellar_relay_lib::{
	sdk::{
		compound_types::{UnlimitedVarArray, XdrArchive},
		types::{ScpEnvelope, ScpHistoryEntry, StellarMessage, TransactionSet},
		XdrCodec,
	},
	StellarOverlayConnection,
};

use crate::oracle::{
	constants::{get_min_externalized_messages, MAX_SLOT_TO_REMEMBER},
	errors::Error,
	types::{EnvelopesMap, LifoMap},
	ScpArchiveStorage, ScpMessageCollector, Slot,
};

/// The Proof of Transactions that needed to be processed
#[derive(Debug, Eq, PartialEq)]
pub struct Proof {
	/// the slot (or ledger) where the transaction is found
	slot: Slot,

	/// the envelopes belonging to the slot
	envelopes: UnlimitedVarArray<ScpEnvelope>,

	/// the transaction set belonging to the slot
	tx_set: TransactionSet,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProofStatus {
	Proof(Proof),
	/// The available ScpEnvelopes are not enough to create a proof, and the slot is too far back
	/// to fetch more.
	LackingEnvelopes,
	/// No ScpEnvelopes found and the slot is too far back.
	NoEnvelopesFound,
	/// TxSet is not found and the slot is too far back.
	NoTxSetFound,
	/// TxSet is fetched again, so wait for it
	WaitForTxSet,
}

impl Proof {
	/// Encodes these Stellar structures to make it easier to send as extrinsic.
	pub fn encode(&self) -> (String, String) {
		let envelopes_xdr = self.envelopes.to_xdr();
		let envelopes_encoded = base64::encode(envelopes_xdr);

		let tx_set_xdr = self.tx_set.to_xdr();
		let tx_set_encoded = base64::encode(tx_set_xdr);

		(envelopes_encoded, tx_set_encoded)
	}

	pub fn slot(&self) -> Slot {
		self.slot
	}

	pub fn envelopes(&self) -> &Vec<ScpEnvelope> {
		self.envelopes.get_vec()
	}

	pub fn tx_set(&self) -> &TransactionSet {
		&self.tx_set
	}
}

// handle missing envelopes
impl ScpMessageCollector {
	async fn fetch_missing_envelopes(&mut self, slot: Slot, sender: &mpsc::Sender<StellarMessage>) {
		let last_slot_index = *self.last_slot_index();

		// If the current slot is still in the range of 'remembered' slots
		if check_slot_position(last_slot_index, slot) {
			self.ask_node_for_envelopes(slot, sender).await;
		} else {
			self.ask_archive_for_envelopes(slot).await;
		}
	}

	async fn ask_node_for_envelopes(&mut self, slot: Slot, sender: &mpsc::Sender<StellarMessage>) {
		// for this slot to be processed, we must put this in our watch list.
		self.watch_slot(slot);
		let _ = sender.send(StellarMessage::GetScpState(slot.try_into().unwrap())).await;
		tracing::info!("requesting to StellarNode for messages of slot {}...", slot);
	}

	async fn ask_archive_for_envelopes(&mut self, slot: Slot) {
		if !self.is_public() {
			// We can only fetch from archives if we are on public network because there are
			// no archive nodes on testnet
			tracing::info!(
				"Not fetching missing envelopes from archive for slot {:?}, because on testnet",
				slot
			);
			return
		}

		self.watch_slot(slot);
		tracing::info!("requesting to Horizon Archive for messages of slot {}...", slot);

		let envelopes_map_lock = self.envelopes_map_clone();
		tokio::spawn(get_envelopes_from_horizon_archive(envelopes_map_lock, slot));
	}
}

// for fetching missing txset
impl ScpMessageCollector {
	/// Fetch txset that are missing in the collector's map.
	async fn fetch_missing_txset(&mut self, slot: Slot, sender: &mpsc::Sender<StellarMessage>) {
		// we need the txset hash to create the message.
		if let Some(txset_hash) = self.get_txset_hash(&slot) {
			tracing::info!(
				"requesting txset for slot {}: {:?}",
				slot,
				String::from_utf8(txset_hash.to_vec())
			);
			let _ = sender.send(StellarMessage::GetTxSet(txset_hash)).await;
		}
	}
}

// handles the creation of proofs.
// this means it will access the maps and potentially the files.
impl ScpMessageCollector {
	/// Returns either a list of ScpEnvelopes or a ProofStatus saying it failed to retrieve a list.
	async fn get_envelopes(
		&mut self,
		slot: Slot,
		sender: &mpsc::Sender<StellarMessage>,
	) -> UnlimitedVarArray<ScpEnvelope> {
		let empty = UnlimitedVarArray::new_empty();

		if let Some(envelopes) = self.envelopes_map().get_with_key(&slot) {
			// lacking envelopes
			if envelopes.len() < get_min_externalized_messages(self.is_public()) {
				tracing::warn!("not enough envelopes to build proof for slot {}", slot);
			} else {
				return UnlimitedVarArray::new(envelopes.clone()).unwrap_or(empty)
			}
		}

		self.fetch_missing_envelopes(slot, sender).await;

		empty
	}

	/// Returns either a TransactionSet or a ProofStatus saying it failed to retrieve the set.
	async fn get_txset(
		&mut self,
		slot: Slot,
		sender: &mpsc::Sender<StellarMessage>,
	) -> Option<TransactionSet> {
		let txset_map = self.txset_map().clone();
		let txset_map = txset_map.get_with_key(&slot).cloned();

		match txset_map {
			None => {
				// If the current slot is still in the range of 'remembered' slots
				if check_slot_position(*self.last_slot_index(), slot) {
					tracing::info!("Wait for TxSet of slot {}", slot);
				} else {
					tracing::warn!("No TxSet found for slot {}", slot);
				}

				self.fetch_missing_txset(slot, sender).await;
				None
			},
			Some(res) => Some(res),
		}
	}

	/// Returns a `ProofStatus`.
	///
	/// # Arguments
	///
	/// * `slot` - The slot of a transactionwe want a proof of.
	/// * `overlay_conn` - The StellarOverlayConnection used for sending messages to Stellar Node
	pub async fn build_proof(
		&mut self,
		slot: Slot,
		sender: &mpsc::Sender<StellarMessage>,
	) -> Option<Proof> {
		let envelopes = self.get_envelopes(slot, sender).await;
		if envelopes.len() == 0 {
			return None
		}

		let tx_set = self.get_txset(slot, sender).await?;

		Some(Proof { slot, envelopes, tx_set })
	}
}

/// Fetching old SCPMessages is only possible if it's not too far back.
fn check_slot_position(last_slot_index: Slot, slot: Slot) -> bool {
	slot > (last_slot_index.saturating_sub(MAX_SLOT_TO_REMEMBER))
}

async fn get_envelopes_from_horizon_archive(
	envelopes_map_lock: Arc<RwLock<EnvelopesMap>>,
	slot: Slot,
) {
	let slot_index: u32 = slot.try_into().unwrap();
	let scp_archive_result = ScpArchiveStorage::get_scp_archive(slot.try_into().unwrap()).await;

	if let Err(e) = scp_archive_result {
		tracing::error!(
			"Could not get SCPArchive for slot {:?} from Horizon Archive: {:?}",
			slot_index,
			e
		);
		return
	}
	let scp_archive: XdrArchive<ScpHistoryEntry> = scp_archive_result.unwrap();

	let value = scp_archive.get_vec().into_iter().find(|&scp_entry| {
		if let ScpHistoryEntry::V0(scp_entry_v0) = scp_entry {
			return scp_entry_v0.ledger_messages.ledger_seq == slot_index
		} else {
			return false
		}
	});

	if let Some(i) = value {
		if let ScpHistoryEntry::V0(scp_entry_v0) = i {
			let slot_scp_envelopes = scp_entry_v0.clone().ledger_messages.messages;
			let vec_scp = slot_scp_envelopes.get_vec().clone(); //TODO store envelopes_map or send via mpsc

			let mut envelopes_map = envelopes_map_lock.write();

			if let None = envelopes_map.get_with_key(&slot) {
				tracing::info!("Adding archived SCP envelopes for slot {}", slot);
				envelopes_map.set_with_key(slot, vec_scp);
			}
		}
	}
}

#[async_trait::async_trait]
pub trait ProofExt: Send + Sync {
	async fn get_proof(&self, slot: Slot) -> Result<ProofStatus, crate::oracle::Error>;

	/// Returns a list of transactions with each of their corresponding proofs
	async fn get_pending_proofs(&self) -> Result<Vec<Proof>, crate::oracle::Error>;

	async fn processed_proof(&self, slot: Slot);
}

#[cfg(test)]
mod test {
	use crate::oracle::collector::proof_builder::check_slot_position;

	#[test]
	fn test_check_slot_position() {
		let last_slot = 100;

		assert!(!check_slot_position(last_slot, 50));
		assert!(!check_slot_position(last_slot, 75));
		assert!(check_slot_position(last_slot, 90));
		assert!(check_slot_position(last_slot, 100));
		assert!(check_slot_position(last_slot, 101));
	}
}
