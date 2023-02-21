use std::{convert::TryInto, future::Future};

use primitives::stellar::types::TransactionHistoryEntry;
use stellar_relay_lib::sdk::{
	compound_types::{UnlimitedVarArray, XdrArchive},
	types::{ScpEnvelope, ScpHistoryEntry, StellarMessage, TransactionSet},
	XdrCodec,
};

use crate::oracle::{
	constants::{get_min_externalized_messages, MAX_SLOT_TO_REMEMBER},
	types::{LifoMap, StellarMessageSender},
	ScpArchiveStorage, ScpMessageCollector, Slot, TransactionsArchiveStorage,
};

/// Returns true if the SCP messages for a given slot are still recoverable from the overlay
/// because the slot is not too far back.
fn check_slot_still_recoverable_from_overlay(last_slot_index: Slot, slot: Slot) -> bool {
	last_slot_index != 0 && slot > (last_slot_index.saturating_sub(MAX_SLOT_TO_REMEMBER))
}

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
	/// fetch envelopes not found in the collector
	async fn fetch_missing_envelopes(&self, slot: Slot, sender: &StellarMessageSender) {
		let last_slot_index = *self.last_slot_index();

		// If the current slot is still in the range of 'remembered' slots
		if check_slot_still_recoverable_from_overlay(last_slot_index, slot) {
			tracing::trace!("fetching missing envelopes of slot {} from Stellar Node...", slot);
			self.ask_node_for_envelopes(slot, sender).await;
		} else {
			tracing::trace!("fetching missing envelopes of slot {} from Archive Node...", slot);
			self.ask_archive_for_envelopes(slot).await;
		}
	}

	/// fetches envelopes from the stellar node
	async fn ask_node_for_envelopes(&self, slot: Slot, sender: &StellarMessageSender) {
		// for this slot to be processed, we must put this in our watch list.
		let _ = sender.send(StellarMessage::GetScpState(slot.try_into().unwrap())).await;
		tracing::debug!("requesting to StellarNode for messages of slot {}...", slot);
	}

	/// fetches envelopes from the archive
	async fn ask_archive_for_envelopes(&self, slot: Slot) {
		if !self.is_public() {
			// Fetch from archives only on public network since no archive nodes
			// are available on testnet
			tracing::debug!(
				"Not fetching missing envelopes from archive for slot {:?}, because on testnet",
				slot
			);
			return
		}

		tokio::spawn(self.get_envelopes_from_horizon_archive(slot));
	}

	/// Returns either a list of ScpEnvelopes
	///
	/// # Arguments
	///
	/// * `slot` - the slot where the needed envelopes are.
	/// * `sender` - used to send messages to Stellar Node
	async fn get_envelopes(
		&self,
		slot: Slot,
		sender: &StellarMessageSender,
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

		// forcefully retrieve envelopes
		self.fetch_missing_envelopes(slot, sender).await;

		empty
	}

	/// Returns either a TransactionSet or a ProofStatus saying it failed to retrieve the set.
	///
	/// # Arguments
	///
	/// * `slot` - the slot where the txset is  to get.
	/// * `sender` - used to send messages to Stellar Node
	async fn get_txset(&self, slot: Slot, sender: &StellarMessageSender) -> Option<TransactionSet> {
		let txset_map = self.txset_map().clone();
		let tx_set = txset_map.get_with_key(&slot).cloned();

		match tx_set {
			Some(res) => Some(res),
			None => {
				// If the current slot is still in the range of 'remembered' slots
				if check_slot_still_recoverable_from_overlay(*self.last_slot_index(), slot) {
					self.fetch_missing_txset_from_overlay(slot, sender).await;
				} else {
					tokio::spawn(self.get_txset_from_horizon_archive(slot));
				}

				None
			},
		}
	}

	/// Send message to overlay network to fetch the missing txset _if_ we already have the txset
	/// hash for it. If we don't have the hash, we can't fetch it from the overlay network.
	async fn fetch_missing_txset_from_overlay(&self, slot: Slot, sender: &StellarMessageSender) {
		// we need the txset hash to create the message.
		if let Some(txset_hash) = self.get_txset_hash(&slot) {
			tracing::debug!("Fetching TxSet for slot {} from overlay", slot);
			if let Err(error) = sender.send(StellarMessage::GetTxSet(txset_hash)).await {
				tracing::error!("failed to send GetTxSet message: {:?}", error);
			}
		}
	}

	/// Returns the Proof
	///
	/// # Arguments
	///
	/// * `slot` - the slot where the txset is  to get.
	/// * `sender` - used to send messages to Stellar Node
	pub async fn build_proof(&self, slot: Slot, sender: &StellarMessageSender) -> Option<Proof> {
		let envelopes = self.get_envelopes(slot, sender).await;
		// return early if we don't have enough envelopes or the tx_set
		if envelopes.len() == 0 {
			return None
		}

		let tx_set = self.get_txset(slot, sender).await?;
		Some(Proof { slot, envelopes, tx_set })
	}

	/// Insert envelopes fetched from the archive to the map
	///
	/// # Arguments
	///
	/// * `envelopes_map_lock` - the map to insert the envelopes to.
	/// * `slot` - the slot where the envelopes belong to
	fn get_envelopes_from_horizon_archive(&self, slot: Slot) -> impl Future<Output = ()> {
		tracing::debug!("Fetching SCP envelopes for slot {} from horizon archive", slot);
		let envelopes_map_arc = self.envelopes_map_clone();
		async move {
			let slot_index: u32 = slot.try_into().unwrap();
			let scp_archive_result =
				ScpArchiveStorage::get_scp_archive(slot.try_into().unwrap()).await;

			if let Err(e) = scp_archive_result {
				tracing::error!(
					"Could not get SCPArchive for slot {:?} from Horizon Archive: {:?}",
					slot_index,
					e
				);
				return
			}
			let scp_archive: XdrArchive<ScpHistoryEntry> = scp_archive_result.unwrap();

			let value = scp_archive.get_vec().iter().find(|&scp_entry| {
				if let ScpHistoryEntry::V0(scp_entry_v0) = scp_entry {
					scp_entry_v0.ledger_messages.ledger_seq == slot_index
				} else {
					false
				}
			});

			if let Some(i) = value {
				if let ScpHistoryEntry::V0(scp_entry_v0) = i {
					let slot_scp_envelopes = scp_entry_v0.clone().ledger_messages.messages;
					let vec_scp = slot_scp_envelopes.get_vec().clone();

					let mut envelopes_map = envelopes_map_arc.write();

					if envelopes_map.get_with_key(&slot).is_none() {
						tracing::debug!("Adding archived SCP envelopes for slot {}", slot);
						envelopes_map.set_with_key(slot, vec_scp);
					}
				}
			}
		}
	}

	/// Inserts txset fetched from the archive to the map
	///
	/// # Arguments
	///
	/// * `txset` - the map to insert the txset to.
	/// * `slot` - the slot where the txset belong to.
	fn get_txset_from_horizon_archive(&self, slot: Slot) -> impl Future<Output = ()> {
		tracing::warn!("Fetching TxSet for slot {} from horizon archive", slot);
		let txset_map_arc = self.txset_map_clone();
		async move {
			let slot_index: u32 = slot.try_into().unwrap();
			let transactions_archive =
				TransactionsArchiveStorage::get_transactions_archive(slot.try_into().unwrap())
					.await;

			if let Err(e) = transactions_archive {
				tracing::error!(
					"Could not get TransactionsArchive for slot {:?} from horizon archive: {:?}",
					slot_index,
					e
				);
				return
			}
			let transactions_archive: XdrArchive<TransactionHistoryEntry> =
				transactions_archive.unwrap();

			let value = transactions_archive
				.get_vec()
				.iter()
				.find(|&entry| entry.ledger_seq == slot_index);

			if let Some(target_history_entry) = value {
				tracing::debug!("Adding archived tx set for slot {}", slot);
				let mut tx_set_map = txset_map_arc.write();
				tx_set_map.set_with_key(slot, target_history_entry.tx_set.clone());
			}
		}
	}
}

#[cfg(test)]
mod test {
	use crate::oracle::collector::proof_builder::check_slot_still_recoverable_from_overlay;

	#[test]
	fn test_check_slot_position() {
		let last_slot = 100;

		assert!(!check_slot_still_recoverable_from_overlay(last_slot, 50));
		assert!(!check_slot_still_recoverable_from_overlay(last_slot, 75));
		assert!(check_slot_still_recoverable_from_overlay(last_slot, 90));
		assert!(check_slot_still_recoverable_from_overlay(last_slot, 100));
		assert!(check_slot_still_recoverable_from_overlay(last_slot, 101));
	}
}
