use std::{convert::TryInto, sync::Arc};

use parking_lot::RwLock;

use stellar_relay_lib::{
	sdk::{
		compound_types::{LimitedVarArray, UnlimitedVarArray, XdrArchive},
		types::{ScpEnvelope, ScpHistoryEntry, StellarMessage, TransactionSet},
		TransactionEnvelope, XdrCodec,
	},
	StellarOverlayConnection,
};

use crate::oracle::{
	constants::{get_min_externalized_messages, MAX_SLOT_TO_REMEMBER},
	traits::FileHandler,
	types::{EnvelopesMap, LifoMap},
	ActorMessage, EnvelopesFileHandler, ScpArchiveStorage, ScpMessageCollector, Slot, TxHash,
	TxSetsFileHandler,
};

/// Determines whether the data retrieved is from the current map or from a file.
type DataFromFile<T> = (T, bool);

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

// for fetching missing info to generate the proof
impl ScpMessageCollector {
	/// Fetch envelopes that are missing in the collector's map.
	///
	/// # Arguments
	///
	/// * `neg_proof_status` - the ProofStatus that will determine what method to use to fetch the
	///   missing envelopes.
	/// * `slot` - the slot where the missng envelopes belong to.
	/// * `overlay_conn` - the overlay connection that connects to the Stellar Node
	async fn fetch_missing_envelopes(
		&mut self,
		neg_proof_status: &ProofStatus,
		slot: Slot,
		overlay_conn: &StellarOverlayConnection,
	) {
		match neg_proof_status {
			// let's fetch from Stellar Node again
			ProofStatus::LackingEnvelopes => {
				// for this slot to be processed, we must put this in our watch list.
				self.watch_slot(slot);
				let _ =
					overlay_conn.send(StellarMessage::GetScpState(slot.try_into().unwrap())).await;
				tracing::info!("requesting to StellarNode for messages of slot {}...", slot);
			},
			// let's get envelopes from horizon
			ProofStatus::NoEnvelopesFound => {
				if !self.is_public() {
					// We can only fetch from archives if we are on public network because there are
					// no archive nodes on testnet
					tracing::info!("Not fetching missing envelopes from archive for slot {:?}, because on testnet", slot);
					return
				}

				self.watch_slot(slot);
				tracing::info!("requesting to Horizon Archive for messages of slot {}...", slot);

				let envelopes_map_lock = self.envelopes_map_clone();
				tokio::spawn(get_envelopes_from_horizon_archive(envelopes_map_lock, slot));
			},
			_ => {},
		}
	}

	/// Fetch txset that are missing in the collector's map.
	async fn fetch_missing_txset(
		&mut self,
		neg_proof_status: &ProofStatus,
		slot: Slot,
		overlay_conn: &StellarOverlayConnection,
	) {
		// if status is "waiting", fetch the txset again from StellarNode
		match neg_proof_status {
			ProofStatus::WaitForTxSet | ProofStatus::NoTxSetFound => {
				// we need the txset hash to create the message.
				if let Some(txset_hash) = self.get_txset_hash(&slot) {
					tracing::info!(
						"requesting txset for slot {}: {:?}",
						slot,
						String::from_utf8(txset_hash.to_vec())
					);
					let _ = overlay_conn.send(StellarMessage::GetTxSet(txset_hash)).await;
				}
			},
			// ProofStatus::NoTxSetFound => {
			//todo: retrieve from TransactionHistoryEntry: https://github.com/pendulum-chain/spacewalk/pull/137
			// },
			_ => {},
		}
	}
}

// handles the creation of proofs.
// this means it will access the maps and potentially the files.
impl ScpMessageCollector {
	/// Returns either a list of ScpEnvelopes or a ProofStatus saying it failed to retrieve a list.
	fn get_envelopes(&self, slot: Slot) -> Result<UnlimitedVarArray<ScpEnvelope>, ProofStatus> {
		let map = self.envelopes_map();
		let envelopes = map.get_with_key(&slot);

		match envelopes {
			None => {
				self.restore_missed_slots(slot);
				Err(ProofStatus::LackingEnvelopes)
			},
			Some(envelopes) => {
				envelopes.len() < get_min_externalized_messages(self.is_public());
				Ok(UnlimitedVarArray::new(envelopes.clone())
					.unwrap_or(UnlimitedVarArray::new_empty()))
			},
		}
	}

	fn restore_missed_slots(&self, slot: Slot) {
		let last_slot_index = *self.last_slot_index();
		let action_sender = self.action_sender.clone();
		let rw_lock = self.envelopes_map_clone();
		tokio::spawn(async move {
			// If the current slot is still in the range of 'remembered' slots
			if slot > last_slot_index - MAX_SLOT_TO_REMEMBER {
				let result =
					action_sender.send(ActorMessage::GetScpState { missed_slot: slot }).await;
			} else {
				let slot_index: u32 = slot.try_into().unwrap();
				let scp_archive: XdrArchive<ScpHistoryEntry> =
					ScpArchiveStorage::get_scp_archive(slot.try_into().unwrap()).await.unwrap();

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

						let mut envelopes_map = rw_lock.write();

						if let None = envelopes_map.get_with_key(&slot) {
							tracing::info!("Adding archived SCP envelopes for slot {}", slot);
							envelopes_map.set_with_key(slot, vec_scp);
						}
					}
				}
			}
		});
	}

	/// helper method for `get_envelopes()`.
	/// It returns a tuple of (list of `ScpEnvelope`s, <if_list_came_from_a_file>).
	fn _get_envelopes(&self, slot: Slot) -> Option<DataFromFile<Vec<ScpEnvelope>>> {
		self.envelopes_map()
			.get_with_key(&slot)
			.map(|envs| (envs.clone(), false))
			.or_else(|| match EnvelopesFileHandler::get_map_from_archives(slot) {
				Ok(env_map) => env_map.get_with_key(&slot).map(|envs| (envs.clone(), true)),
				Err(e) => {
					tracing::warn!("Failed to read envelopes map from a file: {:?}", e);
					None
				},
			})
	}

	/// Returns either a TransactionSet or a ProofStatus saying it failed to retrieve the set.
	fn get_txset(&self, slot: Slot) -> Result<TransactionSet, ProofStatus> {
		match self.txset_map().get_with_key(&slot).map(|set| set.clone()) {
			None => {
				// If the current slot is still in the range of 'remembered' slots
				if check_slot_position(*self.last_slot_index(), slot) {
					return Err(ProofStatus::WaitForTxSet)
				}
				// Use Horizon Archive node to get TransactionHistoryEntry
				Err(ProofStatus::NoTxSetFound)
			},
			Some(res) => Ok(res),
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
		overlay_conn: &StellarOverlayConnection,
	) -> ProofStatus {
		// get the SCPEnvelopes
		let envelopes = match self.get_envelopes(slot) {
			Ok(envelopes) => envelopes,
			Err(neg_status) => {
				self.fetch_missing_envelopes(&neg_status, slot, overlay_conn).await;
				return neg_status
			},
		};

		// get the TransactionSet
		let tx_set = match self.get_txset(slot) {
			Ok(set) => set,
			Err(neg_status) => {
				self.fetch_missing_txset(&neg_status, slot, overlay_conn).await;
				return neg_status
			},
		};

		// a proof has been found. Remove this slot.
		// self.remove_data(&slot);

		ProofStatus::Proof(Proof { slot, envelopes, tx_set })
	}

	/// Returns a list of transactions (only those with proofs) to be processed.
	pub async fn get_pending_proofs(
		&mut self,
		overlay_conn: &StellarOverlayConnection,
	) -> Vec<Proof> {
		let mut proofs_for_handled_txs = Vec::<Proof>::new();

		// let's generate proofs from the pending list.
		for slot in self.read_slot_pending_list().iter() {
			// Try to build proofs
			match self.build_proof(*slot, overlay_conn).await {
				ProofStatus::Proof(proof) => {
					proofs_for_handled_txs.push(proof);
				},
				other => {
					tracing::warn!("cannot build proof for slot {:?}: {:?}", slot, other);
				},
			}
		}

		proofs_for_handled_txs
	}
}

fn return_proper_envelopes_error(last_slot_index: Slot, slot: Slot) -> ProofStatus {
	// If the current slot is still in the range of 'remembered' slots
	if check_slot_position(last_slot_index, slot) {
		return ProofStatus::LackingEnvelopes
	}
	// Use Horizon Archive node to get ScpHistoryEntry
	ProofStatus::NoEnvelopesFound
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
	use crate::oracle::collector::proof_builder::{
		check_slot_position, return_proper_envelopes_error,
	};

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
