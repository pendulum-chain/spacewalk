use primitives::stellar::types::TransactionHistoryEntryExt;
use std::{convert::TryFrom, future::Future};
use tracing::log;

use stellar_relay_lib::sdk::{
	compound_types::{UnlimitedVarArray, XdrArchive},
	types::{ScpEnvelope, ScpHistoryEntry, ScpStatementPledges, StellarMessage},
	InitExt, TransactionSetType, XdrCodec,
};
use wallet::Slot;

use crate::oracle::{
	constants::{get_min_externalized_messages, MAX_SLOTS_TO_REMEMBER},
	traits::ArchiveStorage,
	types::StellarMessageSender,
	ScpArchiveStorage, ScpMessageCollector, TransactionsArchiveStorage,
};

/// The Proof of Transactions that needed to be processed
#[derive(Debug, Eq, PartialEq)]
pub struct Proof {
	/// the slot (or ledger) where the transaction is found
	slot: Slot,

	/// the envelopes belonging to the slot
	envelopes: UnlimitedVarArray<ScpEnvelope>,

	/// the transaction set belonging to the slot
	tx_set: TransactionSetType,
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

	pub fn tx_set(&self) -> &TransactionSetType {
		&self.tx_set
	}
}

// handle missing envelopes
impl ScpMessageCollector {
	/// Returns the Proof
	///
	/// # Arguments
	///
	/// * `slot` - the slot where the txset is  to get.
	/// * `sender` - used to send messages to Stellar Node
	pub async fn build_proof(&self, slot: Slot, sender: &StellarMessageSender) -> Option<Proof> {
		if self.last_slot_index() == 0 {
			tracing::warn!(
				"build_proof(): Proof Building for slot {slot}: last_slot_index is still 0, not yet ready to build proofs."
			);
			return None;
		}

		let Some(envelopes) = self.get_envelopes(slot, sender).await else {
			// return early if we don't have enough envelopes
			tracing::warn!(
				"build_proof(): Couldn't build proof for slot {slot} due to missing envelopes"
			);
			return None;
		};

		let tx_set = self.get_txset(slot, sender).await?;
		Some(Proof { slot, envelopes, tx_set })
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
			// If the data was provided from the archive, no need to check for the minimum
			// Otherwise, we are still lacking envelopes.
			if !self.is_envelopes_data_from_archive(&slot) &&
				envelopes.len() < get_min_externalized_messages(self.is_public())
			{
				tracing::warn!(
					"get_envelopes(): Proof Building for slot {slot}: {:?} envelopes is not enough to build proof",
					envelopes.len()
				);
			} else {
				return UnlimitedVarArray::new(envelopes.clone()).ok();
			}
		}

		// forcefully retrieve envelopes
		self._get_envelopes(slot, sender).await;

		return None;
	}

	/// fetch envelopes not found in the collector
	async fn _get_envelopes(&self, slot: Slot, sender: &StellarMessageSender) {
		tracing::debug!("_get_envelopes(): FOR SLOT {slot} check_slot_still_recoverable_from_overlay: LAST SLOT INDEX: {}",self.last_slot_index());
		// If the current slot is still in the range of 'remembered' slots, retrieve the envelopes
		// from the overlay network
		if check_slot_still_recoverable_from_overlay(self.last_slot_index(), slot) {
			tracing::debug!(
				"_get_envelopes(): Proof Building for slot {slot}: fetching missing envelopes from Stellar Node..."
			);
			self.ask_overlay_for_envelopes(slot, sender).await;

			return;
		}

		tracing::info!(
			"_get_envelopes(): Proof Building for slot {slot}: fetching from Archive Node..."
		);

		self.get_envelopes_from_horizon_archive(slot).await
	}

	/// fetches envelopes from the stellar node
	async fn ask_overlay_for_envelopes(&self, slot: Slot, sender: &StellarMessageSender) {
		// for this slot to be processed, we must put this in our watch list.
		let Ok(slot) = u32::try_from(slot) else {
			tracing::error!(
					"ask_overlay_for_envelopes(): Proof Building for slot {slot:} failed to convert slot value into u32 datatype"
				);
			return;
		};

		if let Err(e) = sender.send(StellarMessage::GetScpState(slot)).await {
			tracing::error!(
				"ask_overlay_for_envelopes(): Proof Building for slot {slot}: failed to send `GetScpState` message: {e:?}"
			);
			return;
		}
		tracing::info!("ask_overlay_for_envelopes(): Proof Building for slot {slot}: requesting to StellarNode for messages...");
	}

	/// Returns a TransactionSet if a txset is found; None if the slot does not have a txset
	///
	/// # Arguments
	///
	/// * `slot` - the slot from where we get the txset
	/// * `sender` - used to send messages to Stellar Node
	async fn get_txset(
		&self,
		slot: Slot,
		sender: &StellarMessageSender,
	) -> Option<TransactionSetType> {
		let txset_map = self.txset_map().clone();
		let tx_set = txset_map.get(&slot).cloned();

		match tx_set {
			Some(res) => Some(res),
			None => {
				tracing::debug!("get_txset(): FOR SLOT {slot} check_slot_still_recoverable_from_overlay: LAST SLOT INDEX: {}",self.last_slot_index());
				// If the current slot is still in the range of 'remembered' slots
				if check_slot_still_recoverable_from_overlay(self.last_slot_index(), slot) {
					self.ask_overlay_for_txset(slot, sender).await;
				} else {
					self.get_txset_from_horizon_archive(slot).await;
				}

				tracing::warn!("get_txset(): Proof Building for slot {slot}: no txset found");
				None
			},
		}
	}

	/// Send message to overlay network to fetch the missing txset _if_ we already have the txset
	/// hash for it. If we don't have the hash, we can't fetch it from the overlay network.
	async fn ask_overlay_for_txset(&self, slot: Slot, sender: &StellarMessageSender) {
		// we need the txset hash to create the message.
		if let Some(txset_hash) = self.get_txset_hash_by_slot(&slot) {
			tracing::debug!("ask_overlay_for_txset(): Proof Building for slot {slot}: Fetching TxSet from overlay...");
			if let Err(error) = sender.send(StellarMessage::GetTxSet(txset_hash)).await {
				tracing::error!("ask_overlay_for_txset(): Proof Building for slot {slot}: failed to send GetTxSet message to overlay {:?}", error);
			}
		}
	}

	/// Insert envelopes fetched from the archive to the map
	///
	/// # Arguments
	///
	/// * `envelopes_map_lock` - the map to insert the envelopes to.
	/// * `slot` - the slot where the envelopes belong to
	fn get_envelopes_from_horizon_archive(&self, slot: Slot) -> impl Future<Output = ()> {
		tracing::debug!("get_envelopes_from_horizon_archive(): Fetching SCP envelopes from horizon archive for slot {slot}...");
		let envelopes_map_arc = self.envelopes_map_clone();
		let env_from_archive_map = self.env_from_archive_map_clone();

		let archive_urls = self.stellar_history_archive_urls();
		async move {
			if archive_urls.is_empty() {
				tracing::error!("get_envelopes_from_horizon_archive(): Cannot get envelopes from horizon archive for slot {slot}: no archive URLs configured");
				return;
			}

			// We try to get the SCPArchive from each archive URL until we succeed or run out of
			// URLs
			for archive_url in archive_urls {
				let scp_archive_storage = ScpArchiveStorage(archive_url.clone());
				let scp_archive_result = scp_archive_storage.get_archive(slot).await;
				if let Err(e) = scp_archive_result {
					tracing::error!(
						"get_envelopes_from_horizon_archive(): Could not get SCPArchive for slot {slot} from Horizon Archive {archive_url}: {e:?}"
					);
					continue;
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

				let Some(ScpHistoryEntry::V0(scp_entry_v0)) = value else {
					tracing::warn!("get_envelopes_from_horizon_archive(): Could not get ScpHistory entry from archive {archive_url} for slot {slot}");
					continue;
				};

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
							"get_envelopes_from_horizon_archive(): The contained archive entry fetched from {} for slot {slot} is invalid because it does not contain any externalized envelopes.",
								scp_archive_storage.0
						);
					// remove the file since it's invalid.
					scp_archive_storage.remove_file(slot);
					continue;
				}

				let mut envelopes_map = envelopes_map_arc.write();
				let mut from_archive_map = env_from_archive_map.write();

				tracing::info!(
							"get_envelopes_from_horizon_archive(): Adding {} archived SCP envelopes for slot {slot} to envelopes map. {} are externalized",
							relevant_envelopes.len(),
							externalized_envelopes_count
						);

				envelopes_map.insert(slot, relevant_envelopes);

				// indicates that the data was taken from the archive
				from_archive_map.insert(slot, ());

				// remove the archive file after successfully retrieving envelopes
				scp_archive_storage.remove_file(slot);
				break;
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
		tracing::info!(
			"get_txset_from_horizon_archive(): Fetching TxSet for slot {slot} from horizon archive"
		);
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
							"get_txset_from_horizon_archive(): Could not get TransactionsArchive for slot {slot} from horizon archive: {e:?}"
						);
						continue;
					},
				};

				let value = transactions_archive
					.get_vec()
					.iter()
					.find(|&entry| Slot::from(entry.ledger_seq) == slot);

				if let Some(target_history_entry) = value {
					tracing::info!(
						"get_txset_from_horizon_archive(): Adding archived tx set for slot {slot}"
					);
					let mut tx_set_map = txset_map_arc.write();

					// Assign the transaction set based on the type defined in `ext`
					let tx_set_type = match target_history_entry.clone().ext {
						TransactionHistoryEntryExt::V1(generalized_tx_set) =>
						// If the type of `ext` is `V1` we use the contained generalized tx set
							TransactionSetType::new(generalized_tx_set),
						// Otherwise we can use the regular `tx_set` contained in the entry
						_ => TransactionSetType::new(target_history_entry.tx_set.clone()),
					};
					tx_set_map.insert(slot, tx_set_type);
					// remove the archive file after a txset has been found
					tx_archive_storage.remove_file(slot);
					break;
				} else {
					tracing::warn!(
						"get_txset_from_horizon_archive(): Could not get TransactionHistory entry from archive for slot {slot}"
					);
				}
			}
		}
	}
}

/// Returns true if the SCP messages for a given slot are still recoverable from the overlay
/// because the slot is not too far back.
fn check_slot_still_recoverable_from_overlay(last_slot_index: Slot, slot: Slot) -> bool {
	let recoverable_point = last_slot_index.saturating_sub(MAX_SLOTS_TO_REMEMBER);
	log::trace!(
		"check_slot_still_recoverable_from_overlay(): Proof Building for slot {slot}: Last Slot to refer to overlay: {recoverable_point}"
	);
	last_slot_index != 0 && slot > recoverable_point
}

#[cfg(test)]
mod test {
	use crate::oracle::{
		collector::{
			proof_builder::check_slot_still_recoverable_from_overlay, ScpMessageCollector,
		},
		types::constants::MAX_SLOTS_TO_REMEMBER,
	};
	use stellar_relay_lib::sdk::types::StellarMessage;
	use tokio::sync::mpsc;

	fn collector(is_mainnet: bool) -> ScpMessageCollector {
		let archives = if is_mainnet {
			vec![
				"http://history.stellar.org/prd/core-live/core_live_001".to_string(),
				"http://history.stellar.org/prd/core-live/core_live_002".to_string(),
				"http://history.stellar.org/prd/core-live/core_live_003".to_string(),
			]
		} else {
			vec![
				"http://history.stellar.org/prd/core-testnet/core_testnet_001".to_string(),
				"http://history.stellar.org/prd/core-testnet/core_testnet_002".to_string(),
				"http://history.stellar.org/prd/core-testnet/core_testnet_003".to_string(),
			]
		};

		ScpMessageCollector::new(is_mainnet, archives)
	}
	#[test]
	fn test_check_slot_position() {
		let last_slot = 50_000;

		assert!(!check_slot_still_recoverable_from_overlay(last_slot, 50));
		assert!(!check_slot_still_recoverable_from_overlay(last_slot, 100));
		assert!(!check_slot_still_recoverable_from_overlay(last_slot, 30_000));
		assert!(!check_slot_still_recoverable_from_overlay(
			last_slot,
			last_slot - MAX_SLOTS_TO_REMEMBER,
		));
		assert!(check_slot_still_recoverable_from_overlay(last_slot, last_slot - 1));
		assert!(check_slot_still_recoverable_from_overlay(
			last_slot,
			last_slot - MAX_SLOTS_TO_REMEMBER + 1,
		));
	}

	#[tokio::test]
	async fn test_ask_overlay_for_envelopes() {
		let (sender, mut receiver) = mpsc::channel::<StellarMessage>(1024);
		let collector = collector(false);

		let expected_slot = 50;
		collector.ask_overlay_for_envelopes(expected_slot, &sender).await;

		match receiver.recv().await.expect("should receive message") {
			StellarMessage::GetScpState(actual_slot) => {
				let actual_slot = u64::from(actual_slot);
				assert_eq!(actual_slot, expected_slot);
			},
			msg => panic!("Expected GetScpState message, got {:?}", msg),
		}
	}

	#[tokio::test]
	async fn test_get_envelopes_from_horizon_archive() {
		let collector = collector(false);
		assert_eq!(collector.envelopes_map_len(), 0);

		let expected_slot = 500;
		let fut = collector.get_envelopes_from_horizon_archive(expected_slot);
		fut.await;

		assert!(collector.envelopes_map_len() > 0);
	}
}
