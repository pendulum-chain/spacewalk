use crate::oracle::ScpArchiveStorage;
use parking_lot::RwLock;
use std::{convert::TryInto, sync::Arc};
use stellar_relay::{
	sdk::{
		compound_types::{UnlimitedVarArray, XdrArchive},
		types::{ScpEnvelope, ScpHistoryEntry, StellarMessage, TransactionSet},
		XdrCodec,
	},
	StellarOverlayConnection,
};

use crate::oracle::{
	constants::{get_min_externalized_messages, MAX_SLOT_TO_REMEMBER},
	types::EnvelopesMap,
	ScpMessageCollector, Slot,
};

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
	/// TxSet is not found and the slot is too far back.
	NoTxSetFound,
	/// TxSet is fetched again, and user just needs to wait for it.
	WaitForTxSet,
}

// handles the creation of proofs.
// this means it will access the maps and potentially the files.
impl ScpMessageCollector {
	/// Returns either a list of ScpEnvelopes or a ProofStatus saying it failed to retrieve a list.
	fn get_envelopes(&self, slot: Slot) -> Result<UnlimitedVarArray<ScpEnvelope>, ProofStatus> {
		match self.envelopes_map().get(&slot) {
			None => {
				// If the current slot is still in the range of 'remembered' slots
				if check_slot_position(*self.last_slot_index(), slot) {
					return Err(ProofStatus::WaitForEnvelopes)
				}
				// Use Horizon Archive node to get ScpHistoryEntry
				Err(ProofStatus::NoEnvelopesFound)
			},
			Some(envelopes) => {
				// lacking envelopes
				if envelopes.len() < get_min_externalized_messages(self.is_public()) {
					// If the current slot is still in the range of 'remembered' slots
					if check_slot_position(*self.last_slot_index(), slot) {
						return Err(ProofStatus::WaitForMoreEnvelopes)
					}
					// Use Horizon Archive node to get ScpHistoryEntry
					return Err(ProofStatus::LackingEnvelopes)
				}

				Ok(UnlimitedVarArray::new(envelopes.clone())
					.unwrap_or(UnlimitedVarArray::new_empty()))
			},
		}
	}

	/// Returns either a TransactionSet or a ProofStatus saying it failed to retrieve the set.
	fn get_txset(&self, slot: Slot) -> Result<TransactionSet, ProofStatus> {
		match self.txset_map().get(&slot).map(|set| set.clone()) {
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
						self.watch_slot(slot);
						let _ = overlay_conn
							.send(StellarMessage::GetScpState(slot.try_into().unwrap()))
							.await;
						tracing::info!(
							"requesting to StellarNode for messages of slot {}...",
							slot
						);
					},
					// let's get envelopes from horizon
					ProofStatus::NoEnvelopesFound => {
						tracing::info!(
							"requesting to Horizon Archive for messages of slot {}...",
							slot
						);

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
			Ok(set) => set,
			Err(neg_status) => {
				// if status is "waiting", fetch the txset again from StellarNode
				if neg_status == ProofStatus::WaitForTxSet {
					// we need the txset hash to create the message.
					if let Some(txset_hash) = self.get_txset_hash(&slot) {
						let _ =
							overlay_conn.send(StellarMessage::GetTxSet(txset_hash.clone())).await;
					}
				}
				return neg_status
			},
		};

		// a proof has been found. Remove this slot.
		self.remove_data(&slot);

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

#[cfg(test)]
mod test {
	use crate::oracle::collector::proof_builder::check_slot_position;

	#[test]
	fn test_check_slot_position() {
		let last_slot = 100;
		let curr_slot = 50;

		let res = check_slot_position(last_slot, curr_slot);
		println!("the result, men: {}", res);
	}
}
