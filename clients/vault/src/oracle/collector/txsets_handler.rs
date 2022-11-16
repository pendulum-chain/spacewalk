use std::sync::Arc;

use parking_lot::RwLock;

use stellar_relay::{helper::compute_non_generic_tx_set_content_hash, sdk::types::TransactionSet};

use crate::oracle::{
	collector::ScpMessageCollector,
	constants::MAX_TXSETS_PER_FILE,
	errors::Error,
	storage::{traits::FileHandlerExt, TxSetsFileHandler},
	types::TxSetMap,
};

impl ScpMessageCollector {
	// /// maps the slot to the transactions of the TransactionSet
	// ///
	// /// # Arguments
	// ///
	// /// * `slot` - the slot of which the tx belongs to
	// /// * `tx_set` - where the txs are derived from.
	// /// * `filter` - filters out transactions (in the Transaction Set) for processing.
	// fn save_transactions_to_map(
	// 	&mut self,
	// 	slot: Slot,
	// 	tx_set: &TransactionSet,
	// 	filters: &TxFilterMap,
	// ) -> Result<(), Error> {
	// 	tracing::info!("Inserting received transaction set for slot {}", slot);
	//
	// 	// Collect tx hashes to save to file.
	// 	tx_set.txes.get_vec().iter().for_each(|tx_env| {
	// 		let tx_hash = tx_env.get_hash(self.network());
	//
	// 		// only relevant transactions are saved.
	// 		if self.is_tx_relevant(tx_env) {
	// 			tracing::info!("saving to {:?} to hash map", base64::encode(&tx_hash));
	// 			self.tx_hash_map_mut().insert(tx_hash, slot);
	// 		}
	//
	// 		// loops through the filters to check if transaction needs to be processed.
	// 		// Add transaction to pending transactions if it is not yet contained
	// 		while let Some(filter) = filters.values().next() {
	// 			if filter.check_for_processing(tx_env) {
	// 				if self
	// 					.pending_transactions
	// 					.iter()
	// 					.find(|(tx, _)| tx.get_hash(self.network()) == tx_hash)
	// 					.is_none()
	// 				{
	// 					self.pending_transactions.push((tx_env.clone(), slot));
	// 					break
	// 				}
	// 			}
	// 		}
	// 	});
	//
	// 	Ok(())
	// }

	/// handles incoming TransactionSet.
	pub(crate) fn handle_tx_set(&mut self, set: &TransactionSet) -> Result<(), Error> {
		// compute the tx_set_hash, to check what slot this set belongs too.
		let tx_set_hash = compute_non_generic_tx_set_content_hash(set);

		// Remove slot from this map and into the pending list
		let slot_opt = self.txset_and_slot_map_mut().remove_by_txset_hash(&tx_set_hash);
		match slot_opt {
			None => {
				tracing::warn!("WARNING! tx_set_hash: {:?} has no slot.", tx_set_hash);
			},
			Some(slot) => {
				self.slot_pendinglist_mut().push(slot);

				// saving a new txset entry
				self.txset_map_mut().insert(slot, set.clone());
			},
		}

		Ok(())
	}

	/// checks whether the transaction set map requires saving to file.
	/// todo: remove this
	fn check_write_tx_set_to_file(&mut self) -> Result<(), Error> {
		// map is too small; we don't have to write it to file just yet.
		if self.txset_map().len() < MAX_TXSETS_PER_FILE {
			return Ok(())
		}

		tracing::info!("saving old transactions to file: {:?}", self.txset_map().keys());

		let filename = TxSetsFileHandler::write_to_file(&self.txset_map())?;
		tracing::info!("new file created: {:?}", filename);

		// see Marcel's comment:
		// https://satoshipay.slack.com/archives/C01V1F56RMJ/p1665130289894279?thread_ts=1665117606.469799&cid=C01V1F56RMJ
		// TxHashesFileHandler::write_to_file(filename, &self.tx_hash_map())?;

		// reset maps
		self.txset_map = Arc::new(RwLock::new(TxSetMap::new()));
		// self.tx_hash_map =  Arc::new(RwLock::new(HashMap::new()));

		Ok(())
	}
}
