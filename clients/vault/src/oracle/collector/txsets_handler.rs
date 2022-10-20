use crate::oracle::{
	collector::ScpMessageCollector,
	constants::MAX_TXSETS_PER_FILE,
	errors::Error,
	storage::{traits::FileHandlerExt, TxSetsFileHandler},
	types::{TxSetCheckerMap, TxSetMap},
	TxFilterMap,
};
use parking_lot::RwLock;
use std::sync::Arc;
use stellar_relay::{helper::compute_non_generic_tx_set_content_hash, sdk::types::TransactionSet};

impl ScpMessageCollector {
	/// handles incoming TransactionSet.
	///
	/// # Arguments
	///
	/// * `set` - the TransactionSet
	/// * `txset_hash_map` - provides the slot number of the given Transaction Set Hash
	/// * `filter` - filters out transactions (in the Transaction Set) for processing.
	pub(crate) fn handle_tx_set(
		&mut self,
		set: &TransactionSet,
		txset_hash_map: &mut TxSetCheckerMap,
		filters: &TxFilterMap,
	) -> Result<(), Error> {
		self.check_write_tx_set_to_file()?;

		// compute the tx_set_hash, to check what slot this set belongs too.
		let tx_set_hash = compute_non_generic_tx_set_content_hash(set);

		// let's remove the entry in `txset_hash_map` once a match is found.
		if let Some(slot) = txset_hash_map.remove(&tx_set_hash) {
			// saving a new txset entry
			self.txset_map_mut().insert(slot, set.clone());

			// save the txs if the txset with the tx's hash as the key.
			self.update_tx_hash_map(slot, set, filters)?;
		} else {
			tracing::warn!("WARNING! tx_set_hash: {:?} has no slot.", tx_set_hash);
		}

		Ok(())
	}

	/// checks whether the transaction set map requires saving to file.
	fn check_write_tx_set_to_file(&mut self) -> Result<(), Error> {
		// map is too small; we don't have to write it to file just yet.
		if self.txset_map().len() < MAX_TXSETS_PER_FILE {
			return Ok(())
		}

		tracing::info!("saving old transactions to file: {:?}", self.txset_map().keys());

		let filename = TxSetsFileHandler::write_to_file(&self.txset_map())?;
		tracing::info!("new file created: {:?}", filename);

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
