use std::{convert::TryFrom, future::Future};
use tracing::log;

use stellar_relay_lib::sdk::{
	compound_types::{UnlimitedVarArray, XdrArchive},
	types::{ScpEnvelope, ScpHistoryEntry, ScpStatementPledges, StellarMessage, TransactionSet},
	XdrCodec,
};

use crate::oracle::{
	constants::{get_min_externalized_messages, MAX_SLOTS_TO_REMEMBER},
	traits::ArchiveStorage,
	types::StellarMessageSender,
	ScpArchiveStorage, ScpMessageCollector, Slot, TransactionsArchiveStorage,
};

/// Returns true if the SCP messages for a given slot are still recoverable from the overlay
/// because the slot is not too far back.
fn check_slot_still_recoverable_from_overlay(last_slot_index: Slot, slot: Slot) -> bool {
	let recoverable_point = last_slot_index.saturating_sub(MAX_SLOTS_TO_REMEMBER);
	log::trace!(
		"Proof Building for slot {slot}: Last Slot to refer to overlay: {recoverable_point}"
	);
	last_slot_index != 0 && slot > recoverable_point
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
		// If the current slot is still in the range of 'remembered' slots
		if check_slot_still_recoverable_from_overlay(self.last_slot_index(), slot) {
			tracing::debug!(
				"Proof Building for slot {slot}: fetching missing envelopes from Stellar Node..."
			);
			self.ask_node_for_envelopes(slot, sender).await;
		} else {
			tracing::debug!(
				"Proof Building for slot {slot}: fetching missing envelopes from Archive Node..."
			);
			self.ask_archive_for_envelopes(slot).await;
		}
	}

	/// fetches envelopes from the stellar node
	async fn ask_node_for_envelopes(&self, slot: Slot, sender: &StellarMessageSender) {
		// for this slot to be processed, we must put this in our watch list.
		let slot = match u32::try_from(slot) {
			Ok(slot) => slot,
			Err(e) => {
				tracing::error!(
					"Proof Building for slot {slot:} failed to convert slot value into u32 datatype: {e:?}"
				);
				return
			},
		};

		if let Err(e) = sender.send(StellarMessage::GetScpState(slot)).await {
			tracing::error!(
				"Proof Building for slot {slot}: failed to send `GetScpState` message: {e:?}"
			);
			return
		}
		tracing::debug!(
			"Proof Building for slot {slot}: requesting to StellarNode for messages..."
		);
	}

	/// fetches envelopes from the archive
	async fn ask_archive_for_envelopes(&self, slot: Slot) {
		if !self.is_public() {
			// Fetch from archives only on public network since no archive nodes
			// are available on testnet
			tracing::debug!(
				"Proof Building for slot {slot}: Cannot fetch envelopes from archive for test network"
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
	) -> Option<UnlimitedVarArray<ScpEnvelope>> {
		if let Some(envelopes) = self.envelopes_map().get(&slot) {
			// lacking envelopes
			if envelopes.len() < get_min_externalized_messages(self.is_public()) {
				tracing::debug!(
					"Proof Building for slot {slot}: not enough envelopes to build proof "
				);
			} else {
				return UnlimitedVarArray::new(envelopes.clone()).ok()
			}
		}

		// forcefully retrieve envelopes
		self.fetch_missing_envelopes(slot, sender).await;

		return None
	}

	/// Returns a TransactionSet if a txset is found; None if the slot does not have a txset
	///
	/// # Arguments
	///
	/// * `slot` - the slot from where we get the txset
	/// * `sender` - used to send messages to Stellar Node
	async fn get_txset(&self, slot: Slot, sender: &StellarMessageSender) -> Option<TransactionSet> {
		let txset_map = self.txset_map().clone();
		let tx_set = txset_map.get(&slot).cloned();

		match tx_set {
			Some(res) => Some(res),
			None => {
				// If the current slot is still in the range of 'remembered' slots
				if check_slot_still_recoverable_from_overlay(self.last_slot_index(), slot) {
					self.fetch_missing_txset_from_overlay(slot, sender).await;
				} else {
					tokio::spawn(self.get_txset_from_horizon_archive(slot));
				}

				tracing::debug!("Proof Building for slot {slot}: no txset found");
				None
			},
		}
	}

	/// Send message to overlay network to fetch the missing txset _if_ we already have the txset
	/// hash for it. If we don't have the hash, we can't fetch it from the overlay network.
	async fn fetch_missing_txset_from_overlay(&self, slot: Slot, sender: &StellarMessageSender) {
		// we need the txset hash to create the message.
		if let Some(txset_hash) = self.get_txset_hash_by_slot(&slot) {
			tracing::debug!("Proof Building for slot {slot}: Fetching TxSet from overlay...");
			if let Err(error) = sender.send(StellarMessage::GetTxSet(txset_hash)).await {
				tracing::error!("Proof Building for slot {slot}: failed to send GetTxSet message to overlay {:?}", error);
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
		let envelopes_maybe = self.get_envelopes(slot, sender).await;
		// return early if we don't have enough envelopes or the tx_set
		if let Some(envelopes) = envelopes_maybe {
			let tx_set = self.get_txset(slot, sender).await?;
			return Some(Proof { slot, envelopes, tx_set })
		} else {
			tracing::debug!("Couldn't build proof for slot {slot} due to missing envelopes");
			return None
		}
	}

	/// Insert envelopes fetched from the archive to the map
	///
	/// # Arguments
	///
	/// * `envelopes_map_lock` - the map to insert the envelopes to.
	/// * `slot` - the slot where the envelopes belong to
	fn get_envelopes_from_horizon_archive(&self, slot: Slot) -> impl Future<Output = ()> {
		tracing::debug!("Fetching SCP envelopes from horizon archive for slot {slot}...");
		let envelopes_map_arc = self.envelopes_map_clone();

		let archive_urls = self.stellar_history_archive_urls();
		async move {
			if archive_urls.is_empty() {
				tracing::error!("Cannot get envelopes from horizon archive for slot {slot}: no archive URLs configured");
				return
			}

			// We try to get the SCPArchive from each archive URL until we succeed or run out of
			// URLs
			for archive_url in archive_urls {
				let scp_archive_storage = ScpArchiveStorage(archive_url);
				let scp_archive_result = scp_archive_storage.get_archive(slot).await;
				if let Err(e) = scp_archive_result {
					tracing::error!(
						"Could not get SCPArchive for slot {slot} from Horizon Archive: {e:?}"
					);
					continue
				}
				let scp_archive: XdrArchive<ScpHistoryEntry> =
					scp_archive_result.expect("Should unwrap SCPArchive");

				let value = scp_archive.get_vec().iter().find(|&scp_entry| {
					if let ScpHistoryEntry::V0(scp_entry_v0) = scp_entry {
						Slot::from(scp_entry_v0.ledger_messages.ledger_seq) == slot
					} else {
						false
					}
				});

				if let Some(i) = value {
					if let ScpHistoryEntry::V0(scp_entry_v0) = i {
						let slot_scp_envelopes = scp_entry_v0.clone().ledger_messages.messages;
						let vec_scp = slot_scp_envelopes.get_vec().clone();

						// Filter out any envelopes that are not externalize or confirm statements
						let relevant_envelopes = vec_scp
							.into_iter()
							.filter(|scp| match scp.statement.pledges {
								ScpStatementPledges::ScpStExternalize(_) |
								ScpStatementPledges::ScpStConfirm(_) => true,
								_ => false,
							})
							.collect::<Vec<_>>();

						let externalized_envelopes_count = relevant_envelopes
							.iter()
							.filter(|scp| match scp.statement.pledges {
								ScpStatementPledges::ScpStExternalize(_) => true,
								_ => false,
							})
							.count();

						// Ensure that at least one envelope is externalized
						if externalized_envelopes_count == 0 {
							tracing::error!(
							"The contained archive entry fetched from {} for slot {slot} is invalid because it does not contain any externalized envelopes.",
								scp_archive_storage.0
						);
							continue
						}

						let mut envelopes_map = envelopes_map_arc.write();

						if envelopes_map.get(&slot).is_none() {
							tracing::debug!(
								"Adding {} archived SCP envelopes for slot {slot} to envelopes map. {} are externalized",
								relevant_envelopes.len(),
								externalized_envelopes_count
							);
							envelopes_map.insert(slot, relevant_envelopes);
							break
						}
					}
				} else {
					tracing::warn!("Could not get ScpHistory entry from archive for slot {slot}");
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
		tracing::info!("Fetching TxSet for slot {slot} from horizon archive");
		let txset_map_arc = self.txset_map_clone();
		let archive_urls = self.stellar_history_archive_urls();

		async move {
			for archive_url in archive_urls {
				let tx_archive_storage = TransactionsArchiveStorage(archive_url);
				let transactions_archive = tx_archive_storage.get_archive(slot).await;

				let transactions_archive = match transactions_archive {
					Ok(value) => value,
					Err(e) => {
						tracing::error!(
							"Could not get TransactionsArchive for slot {slot} from horizon archive: {e:?}"
						);
						continue
					},
				};

				let value = transactions_archive
					.get_vec()
					.iter()
					.find(|&entry| Slot::from(entry.ledger_seq) == slot);

				if let Some(target_history_entry) = value {
					tracing::debug!("Adding archived tx set for slot {slot}");
					let mut tx_set_map = txset_map_arc.write();
					tx_set_map.insert(slot, target_history_entry.tx_set.clone());
					break
				} else {
					tracing::warn!(
						"Could not get TransactionHistory entry from archive for slot {slot}"
					);
				}
			}
		}
	}
}

#[cfg(test)]
mod test {
	use crate::oracle::collector::proof_builder::check_slot_still_recoverable_from_overlay;

	#[test]
	fn test_check_slot_position() {
		let last_slot = 50_000;

		assert!(!check_slot_still_recoverable_from_overlay(last_slot, 50));
		assert!(!check_slot_still_recoverable_from_overlay(last_slot, 100));
		assert!(!check_slot_still_recoverable_from_overlay(last_slot, 30_000));
		assert!(check_slot_still_recoverable_from_overlay(last_slot, 40_500));
		assert!(check_slot_still_recoverable_from_overlay(last_slot, 49_500));
	}
}
