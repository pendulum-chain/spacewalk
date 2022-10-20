use crate::oracle::{
	traits::FileHandler, EnvelopesFileHandler, ScpMessageCollector, Slot, TxHash, TxSetsFileHandler,
};
use stellar_relay::sdk::{
	compound_types::UnlimitedVarArray,
	types::{ScpEnvelope, TransactionSet},
	TransactionEnvelope, XdrCodec,
};

/// Determines whether the data retrieved is from the current map or from a file.
type DataFromFile<T> = (T, bool);

/// The Proof of Transactions that needed to be processed
pub struct EncodedProof {
	tx_env: String,
	envelopes: String,
	tx_set: String,
}

pub enum ProofStatus {
	Proof(EncodedProof),
	LackingEnvelopes,
	NoEnvelopesFound(Slot),
	NoTxSetFound(Slot),
}

// handles the creation of proofs.
// this means it will access the maps and potentially the files.
impl ScpMessageCollector {
	fn get_slot(&self, tx_hash: &TxHash) -> Option<Slot> {
		self.tx_hash_map().get(tx_hash).map(|slot| *slot)
	}

	/// Returns Either a list of ScpEnvelopes or a ProofStatus saying it failed to retrieve a list.
	fn get_envelopes(&self, slot: Slot) -> Result<UnlimitedVarArray<ScpEnvelope>, ProofStatus> {
		let (envelopes, is_from_file) =
			self._get_envelopes(slot).ok_or(ProofStatus::NoEnvelopesFound(slot))?;

		if self.is_public() {
			// if the list does not come from a file, meaning there's still a chance to get more
			// envelopes.
			if envelopes.len() < 20 && !is_from_file {
				tracing::warn!("Not yet enough envelopes to build proof, current amount {:?}. Retrying in next loop...", envelopes.len());
				return Err(ProofStatus::LackingEnvelopes)
			}
		} else if envelopes.len() < 2 && !is_from_file {
			tracing::warn!("Not yet enough envelopes to build proof, current amount {:?}. Retrying in next loop...", envelopes.len());
			return Err(ProofStatus::LackingEnvelopes)
		}

		Ok(UnlimitedVarArray::new(envelopes).unwrap_or(UnlimitedVarArray::new_empty()))
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

	/// Returns Either a list of ScpEnvelopes or a ProofStatus saying it failed to retrieve a list.
	fn get_txset(&self, slot: Slot) -> Result<DataFromFile<TransactionSet>, ProofStatus> {
		self.txset_map()
			.get(&slot)
			.map(|set| (set.clone(), false))
			.or_else(|| match TxSetsFileHandler::get_map_from_archives(slot) {
				Ok(set_map) => set_map.get(&slot).map(|set| (set.clone(), true)),
				Err(_) => None,
			})
			.ok_or(ProofStatus::NoTxSetFound(slot))
	}

	/// Returns a `ProofStatus`.
	///
	/// # Arguments
	///
	/// * `tx_env` - the TransactionEnvelope to buid a proof of.
	/// * `slot` - The expected slot where the `tx_env` belongs to.
	pub(super) fn build_proof(&self, tx_env: TransactionEnvelope, slot: Slot) -> ProofStatus {
		let envelopes = match self.get_envelopes(slot) {
			Ok(envelopes) => envelopes,
			Err(neg_status) => return neg_status,
		};

		let tx_set = match self.get_txset(slot) {
			Ok((tx_set, _)) => tx_set,
			Err(neg_status) => return neg_status,
		};

		let (tx_env, envelopes, tx_set) = encode(tx_env, envelopes, tx_set);

		ProofStatus::Proof(EncodedProof { tx_env, envelopes, tx_set })
	}
}

/// Encodes these Stellar structures to make it easier to send as extrinsic.
fn encode(
	tx_env: TransactionEnvelope,
	envelopes: UnlimitedVarArray<ScpEnvelope>,
	tx_set: TransactionSet,
) -> (String, String, String) {
	let tx_env_xdr = tx_env.to_xdr();
	let tx_env_encoded = base64::encode(tx_env_xdr);

	let envelopes_xdr = envelopes.to_xdr();
	let envelopes_encoded = base64::encode(envelopes_xdr);

	let tx_set_xdr = tx_set.to_xdr();
	let tx_set_encoded = base64::encode(tx_set_xdr);

	(tx_env_encoded, envelopes_encoded, tx_set_encoded)
}
