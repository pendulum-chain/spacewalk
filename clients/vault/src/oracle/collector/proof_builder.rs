use crate::oracle::ScpArchiveStorage;
use parking_lot::RwLock;
use std::{convert::TryInto, iter::FromIterator, sync::Arc};
use stellar_relay::{
	sdk::{
		compound_types::{LimitedVarArray, UnlimitedVarArray, XdrArchive},
		types::{ScpEnvelope, ScpHistoryEntry, StellarMessage, TransactionSet},
		TransactionEnvelope, XdrCodec,
	},
	StellarOverlayConnection,
};

use crate::oracle::{
	constants::MAX_SLOT_TO_REMEMBER, traits::FileHandler, types::EnvelopesMap,
	EnvelopesFileHandler, ScpMessageCollector, Slot, TxSetsFileHandler,
};

/// Determines whether the data retrieved is from the current map or from a file.
type DataFromFile<T> = (T, bool);

/// The Proof of Transactions that needed to be processed
#[derive(Debug, Eq, PartialEq)]
pub struct Proof {
	slot: Slot,
	envelopes: UnlimitedVarArray<ScpEnvelope>,
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
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProofStatus {
	Proof(Proof),
	/// The available ScpEnvelopes are not enough to create a proof, and the slot is too far back
	/// to fetch more.
	LackingEnvelopes,
	/// More envelopes have been requested from the Stellar Node, so user needs to wait.
	WaitForMoreEnvelopes,
	/// No ScpEnvelopes found and the slot is too far back.
	NoEnvelopesFound,
	/// Envelopes is fetched again, and user just needs to wait for it.
	WaitForEnvelopes,
	/// TxSet is not found and will not be fetched again since the slot is too far back.
	NoTxSetFound,
	/// TxSet is fetched again, and user just needs to wait for it.
	WaitForTxSet,
}

// handles the creation of proofs.
// this means it will access the maps and potentially the files.
impl ScpMessageCollector {
	/// Returns either a list of ScpEnvelopes or a ProofStatus saying it failed to retrieve a list.
	fn get_envelopes(&self, slot: Slot) -> Result<UnlimitedVarArray<ScpEnvelope>, ProofStatus> {
		let envelopes = self._get_envelopes(slot);

		let mut vec_envelopes = vec![];

		// there's no record of this slot in the storage
		if envelopes.is_none() {
			// If the current slot is still in the range of 'remembered' slots
			if check_slot_position(*self.last_slot_index(), slot) {
				return Err(ProofStatus::WaitForEnvelopes)
			}
			// Use Horizon Archive node to get ScpHistoryEntry
			return Err(ProofStatus::NoEnvelopesFound)
		}

		vec_envelopes = envelopes.unwrap().0;

		// lacking envelopes
		if vec_envelopes.len() < self.min_externalized_messages() {
			// If the current slot is still in the range of 'remembered' slots
			if check_slot_position(*self.last_slot_index(), slot) {
				return Err(ProofStatus::WaitForMoreEnvelopes)
			}
			// Use Horizon Archive node to get ScpHistoryEntry
			return Err(ProofStatus::LackingEnvelopes)
		}

		Ok(UnlimitedVarArray::new(vec_envelopes).unwrap_or(UnlimitedVarArray::new_empty()))
	}

	/// helper method for `get_envelopes()`.
	/// It returns a tuple of (list of `ScpEnvelope`s, <if_list_came_from_a_file>).
	fn _get_envelopes(&self, slot: Slot) -> Option<DataFromFile<Vec<ScpEnvelope>>> {
		self.envelopes_map().get(&slot).map(|envs| (envs.clone(), false)).or_else(|| {
			match EnvelopesFileHandler::get_map_from_archives(slot) {
				Ok(env_map) => env_map.get(&slot).map(|envs| (envs.clone(), true)),
				Err(e) => {
					tracing::warn!("Failed to read envelopes map from a file: {:?}", e);
					None
				},
			}
		})
	}

	/// Returns either a TransactionSet or a ProofStatus saying it failed to retrieve the set.
	fn get_txset(&self, slot: Slot) -> Result<DataFromFile<TransactionSet>, ProofStatus> {
		let res = self.txset_map().get(&slot).map(|set| (set.clone(), false)).or_else(|| {
			match TxSetsFileHandler::get_map_from_archives(slot) {
				Ok(set_map) => set_map.get(&slot).map(|set| (set.clone(), true)),
				Err(_) => None,
			}
		});

		match res {
			None => {
				// If the current slot is still in the range of 'remembered' slots
				if check_slot_position(*self.last_slot_index(), slot) {
					return Err(ProofStatus::WaitForTxSet)
				}

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
				match &neg_status {
					// let's fetch from Stellar Node again
					ProofStatus::WaitForMoreEnvelopes | ProofStatus::WaitForEnvelopes => {
						let _ = overlay_conn
							.send(StellarMessage::GetScpState(slot.try_into().unwrap()))
							.await;
					},
					ProofStatus::NoEnvelopesFound => {
						let rw_lock = self.envelopes_map_clone();
						tokio::spawn(get_envelopes_from_horizon_archive(rw_lock, slot));
					},
					_ => {},
				}
				return neg_status
			},
		};

		// get the TransactionSet
		let tx_set = match self.get_txset(slot) {
			Ok((tx_set, _)) => tx_set,
			Err(neg_status) => {
				if neg_status == ProofStatus::WaitForTxSet {
					let txset_and_slot_map = self.txset_and_slot_map().clone();

					if let Some(txset_hash) = txset_and_slot_map.get_txset_hash(&slot) {
						let _ =
							overlay_conn.send(StellarMessage::GetTxSet(txset_hash.clone())).await;
					}
				}

				return neg_status
			},
		};

		// removing this slot from the list of slots to watch.
		self.slot_watchlist_mut().remove(&slot);

		ProofStatus::Proof(Proof { slot, envelopes, tx_set })
	}

	/// Returns a list of transactions (only those with proofs) to be processed.
	pub async fn get_pending_proofs(
		&mut self,
		overlay_conn: &StellarOverlayConnection,
	) -> Vec<Proof> {
		// Store the handled transaction indices in a vec to be able to remove them later
		let mut handled_tx_indices = Vec::new();

		let mut proofs_for_handled_txs = Vec::<Proof>::new();

		let slot_pendinglist = self.slot_pendinglist().clone();
		for (index, slot) in slot_pendinglist.iter().enumerate() {
			// Try to build proofs
			match self.build_proof(*slot, overlay_conn).await {
				ProofStatus::Proof(proof) => {
					// a proof has been found. Remove this slot.
					self.slot_watchlist_mut().remove(slot);

					handled_tx_indices.push(index);
					proofs_for_handled_txs.push(proof);
				},
				_ => {},
			}
		}
		// Remove the transactions with proofs from the pending list.
		for index in handled_tx_indices.iter().rev() {
			self.slot_pendinglist_mut().remove(*index);
		}

		proofs_for_handled_txs
	}
}

/// Fetching old SCPMessages is only possible if it's not too far back.
fn check_slot_position(last_slot_index: Slot, slot: Slot) -> bool {
	slot > (last_slot_index - MAX_SLOT_TO_REMEMBER)
}

async fn get_envelopes_from_horizon_archive(
	envelopes_map_lock: Arc<RwLock<EnvelopesMap>>,
	slot: Slot,
) {
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

			let mut envelopes_map = envelopes_map_lock.write();

			if let None = envelopes_map.get_mut(&slot) {
				tracing::info!("Adding archived SCP envelopes for slot {}", slot);
				envelopes_map.insert(slot, vec_scp);
			}
		}
	}
}
