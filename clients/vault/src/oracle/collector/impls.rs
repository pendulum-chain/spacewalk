use parking_lot::RwLock;
use std::sync::Arc;
use tracing::log;
use stellar_relay::{
	helper::compute_non_generic_tx_set_content_hash,
	sdk::{
		types::{PaymentOp, ScpEnvelope, ScpStatementPledges, StellarMessage, TransactionSet},
		Transaction, TransactionEnvelope,
	},
	StellarOverlayConnection,
};
use crate::oracle::collector::{check_memo, EncodedProof, get_tx_set_hash, ProofStatus, ScpMessageCollector};
use crate::oracle::constants::{MAX_SLOTS_PER_FILE, MAX_TXSETS_PER_FILE, MIN_EXTERNALIZED_MESSAGES};
use crate::oracle::errors::Error;
use crate::oracle::storage::{EnvelopesFileHandler, TxSetsFileHandler};
use crate::oracle::TxHandler;
use crate::oracle::types::{Slot, TxSetCheckerMap, TxSetMap};


// the maximum distance of the selected slot from the current slot.
// this is primarily used when deciding to move maps to a file.
const MAX_DISTANCE_FROM_CURRENT_SLOT: Slot = 3;

// Handling SCPEnvelopes
impl ScpMessageCollector {
	/// handles incoming ScpEnvelope.
	///
	/// # Arguments
	///
	/// * `env` - the ScpEnvelope
	/// * `txset_hash_map` - provides the slot number of the given Transaction Set Hash
	/// * `overlay_conn` - The StellarOverlayConnection used for sending messages to Stellar Node
	pub(crate) async fn handle_envelope(
		&mut self,
		env: ScpEnvelope,
		txset_hash_map: &mut TxSetCheckerMap,
		overlay_conn: &StellarOverlayConnection,
	) -> Result<(), Error> {
		let slot = env.statement.slot_index;

		// we are only interested with `ScpStExternalize`. Other messages are ignored.
		if let ScpStatementPledges::ScpStExternalize(stmt) = &env.statement.pledges {
			let txset_hash = get_tx_set_hash(stmt)?;

			// check for any new entries to insert
			if txset_hash_map.get(&txset_hash).is_none() &&
                // also check whether this is a delayed message.
                !self.txset_map().contains_key(&slot)
			{
				txset_hash_map.insert(txset_hash, slot);
				// let's request the txset from Stellar Node
				overlay_conn.send(StellarMessage::GetTxSet(txset_hash)).await?;

				// check if we need to transfer the map to a file
				self.check_write_envelopes_to_file(slot)?;
			}

			// insert/add the externalized message to map.
			let mut envelopes_map = self.envelopes_map_mut();

			if let Some(value) = envelopes_map.get_mut(&slot) {
				value.push(env);
			} else {
				log::info!("Adding received SCP envelopes for slot {}", slot);
				envelopes_map.insert(slot, vec![env]);
			}
		}

		Ok(())
	}

	/// Checks whether the envelopes map requires saving to file.
	/// One of the factors would be "how old" the slot is, against the current slot.
	fn check_write_envelopes_to_file(&mut self, current_slot: Slot) -> Result<(), Error> {
		let env_map = self.envelopes_map().clone();
		let mut slots = env_map.keys();
		let slots_len = slots.len();

		// map is too small; we don't have to write it to file just yet.
		if slots_len < MAX_SLOTS_PER_FILE {
			return Ok(())
		}

		log::debug!("Size of envelopes map exceeds limit. Writing it to file...");

		let mut counter = 0;
		while let Some(slot) = slots.next() {
			// ends the loop and saves the map to a file.
			if counter == MAX_SLOTS_PER_FILE {
				self.write_envelopes_to_file(*slot)?;
				break
			}

			if let Some(value) = env_map.get(slot) {
				// check if we have enough externalized messages for the corresponding key
				if value.len() < MIN_EXTERNALIZED_MESSAGES
                    // if the key is too far back from the current slot,
                    // then we don't need to wait for more messages.
                    && (current_slot - *slot) < MAX_DISTANCE_FROM_CURRENT_SLOT
				{
					log::warn!("slot: {} not enough messages. Let's wait for more.", slot);
					break
				}
			} else {
				log::error!("something wrong??? race condition?");
				// something wrong??? race condition?
				break
			}

			counter +=1;

		}

		Ok(())
	}

	/// saves a portion/or everything of the `envelopes_map` to a file.
	fn write_envelopes_to_file(&mut self, last_slot: Slot) -> Result<(), Error> {
		let new_slot_map = self.envelopes_map_mut().split_off(&last_slot);
		let res = EnvelopesFileHandler::write_to_file(&self.envelopes_map())?;
		log::info!("writing old envelopes map to file: {}",res);
		self.envelopes_map = Arc::new(RwLock::new(new_slot_map));
		log::debug!("Oldest slot # is: {:?}", last_slot);

		Ok(())
	}
}

// Handling TxSets
impl ScpMessageCollector {
	/// handles incoming TransactionSet.
	///
	/// # Arguments
	///
	/// * `set` - the TransactionSet
	/// * `txset_hash_map` - provides the slot number of the given Transaction Set Hash
	/// * `filter` - filters out transactions (in the Transaction Set) for processing.
	pub(crate) async fn handle_tx_set(
		&mut self,
		set: &TransactionSet,
		txset_hash_map: &mut TxSetCheckerMap,
		filter: &(dyn TxHandler<TransactionEnvelope> + Send + Sync),
	) -> Result<(), Error> {
		self.check_write_tx_set_to_file()?;

		// compute the tx_set_hash, to check what slot this set belongs too.
		let tx_set_hash = compute_non_generic_tx_set_content_hash(set);

		// let's remove the entry in `txset_hash_map` once a match is found.
		if let Some(slot) = txset_hash_map.remove(&tx_set_hash) {
			// saving a new txset entry
			self.txset_map_mut().insert(slot, set.clone());

			// save the txs if the txset with the tx's hash as the key.
			self.update_tx_hash_map(slot, set, filter).await?;
		} else {
			log::warn!("WARNING! tx_set_hash: {:?} has no slot.", tx_set_hash);
		}

		Ok(())
	}

	/// checks whether the transaction set map requires saving to file.
	fn check_write_tx_set_to_file(&mut self) -> Result<(), Error> {
		// map is too small; we don't have to write it to file just yet.
		if self.txset_map().len() < MAX_TXSETS_PER_FILE {
			return Ok(())
		}

		log::info!("saving old transactions to file: {:?}", self.txset_map().keys());

		let filename = TxSetsFileHandler::write_to_file(&self.txset_map())?;
		log::info!("new file created: {:?}", filename);

		// todo: how to appropriately store the tx_hash_map that would make lookup easier?
		// see Marcel's comment:
		// https://satoshipay.slack.com/archives/C01V1F56RMJ/p1665130289894279?thread_ts=1665117606.469799&cid=C01V1F56RMJ
		// TxHashesFileHandler::write_to_file(filename, &self.tx_hash_map())?;

		// reset maps
		self.txset_map = Arc::new(RwLock::new(TxSetMap::new()));
		// self.tx_hash_map =  Arc::new(RwLock::new(HashMap::new()));

		Ok(())
	}
}

// handling transactions
impl ScpMessageCollector {
	/// maps the slot to the transactions of the TransactionSet
	///
	/// # Arguments
	///
	/// * `slot` - the slot of which the tx belongs to
	/// * `tx_set` - where the txs are derived from.
	/// * `filter` - filters out transactions (in the Transaction Set) for processing.
	async fn update_tx_hash_map(
		&mut self,
		slot: Slot,
		tx_set: &TransactionSet,
		filter: &(dyn TxHandler<TransactionEnvelope> + Send + Sync),
	) -> Result<(), Error> {
		log::info!("Inserting received transaction set for slot {}", slot);

		// Collect tx hashes to save to file.
		tx_set.txes.get_vec().iter().for_each(|tx_env| {
			let tx_hash = tx_env.get_hash(self.network());

			// only relevant transactions are saved.
			if self.is_tx_relevant(tx_env) {
				self.tx_hash_map_mut().insert(tx_hash, slot);
			}

			// check if we need to process this transaction.
			// Add transaction to pending transactions if it is not yet contained
			if filter.process_tx(tx_env) {
				if self
					.pending_transactions
					.iter()
					.find(|(tx, _)| tx.get_hash(self.network()) == tx_hash)
					.is_none()
				{
					self.pending_transactions.push((tx_env.clone(), slot));
				}
			}
		});

		Ok(())
	}

	/// Returns a list of transactions (only those with proofs) to be processed.
	pub fn get_pending_txs(&mut self) -> Vec<EncodedProof> {
		// Store the handled transaction indices in a vec to be able to remove them later
		let mut handled_tx_indices = Vec::new();

		let mut handled_txs = Vec::new();

		for (index, (tx_env, slot)) in self.pending_transactions.iter().enumerate() {
			// Try to build proofs
			match self.build_proof(tx_env.clone(), *slot) {
				ProofStatus::Proof(proof) => {
					handled_tx_indices.push(index);
					handled_txs.push(proof);
				},
				_ => {},
			}
		}
		// Remove the transactions with proofs from the pending list.
		for index in handled_tx_indices.iter().rev() {
			self.pending_transactions.remove(*index);
		}

		handled_txs
	}
}

// Checking for Tx Relevance
impl ScpMessageCollector {

	/// Checks whether the transaction is relevant to this vault.
	fn is_tx_relevant(&self, transaction_env: &TransactionEnvelope) -> bool {
		match transaction_env {
			TransactionEnvelope::EnvelopeTypeTx(value) =>
				self._is_tx_relevant(&value.tx) && check_memo(&value.tx.memo),
			_ => false,
		}
	}

	/// helper method of `is_tx_relevant()`
	fn _is_tx_relevant(&self, transaction: &Transaction) -> bool {
		let payment_ops_to_vault_address: Vec<&PaymentOp> = transaction
			.operations
			.get_vec()
			.into_iter()
			.filter_map(|op| match &op.body {
				stellar_relay::sdk::types::OperationBody::Payment(p) => {
					let d = p.destination.clone();
					if self.vault_addresses.contains(
						&String::from_utf8(Vec::from(d.to_encoding().as_slice())).unwrap(),
					) {
						Some(p)
					} else {
						None
					}
				},
				_ => None,
			})
			.collect();

		if payment_ops_to_vault_address.len() == 0 {
			// The transaction is not relevant to use since it doesn't
			// include a payment to our vault address
			return false
		} else {
			log::info!("Transaction to our vault address received.");
			let source = transaction.source_account.clone();
			for payment_op in payment_ops_to_vault_address {
				let destination = payment_op.destination.clone();
				// let amount = payment_op.amount;
				// let asset = payment_op.asset.clone();
				// log::info!("Deposit amount {:?} stroops", amount);
				// print_asset(asset);
				log::info!(
					"From {:#?}",
					std::str::from_utf8(source.to_encoding().as_slice()).unwrap()
				);
				log::info!(
					"To {:?}",
					std::str::from_utf8(destination.to_encoding().as_slice()).unwrap()
				);
			}
			return true
		}
	}
}
