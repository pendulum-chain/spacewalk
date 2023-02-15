use crate::oracle::{
	collector::{get_tx_set_hash, ScpMessageCollector},
	errors::Error,
	types::StellarMessageSender,
};
use stellar_relay_lib::{
	helper::compute_non_generic_tx_set_content_hash,
	sdk::types::{ScpEnvelope, ScpStatementPledges, StellarMessage, TransactionSet},
};

// Handling SCPEnvelopes
impl ScpMessageCollector {
	/// handles incoming ScpEnvelope.
	///
	/// # Arguments
	///
	/// * `env` - the ScpEnvelope
	/// * `message_sender` - used for sending messages to Stellar Node
	pub(crate) async fn handle_envelope(
		&mut self,
		env: ScpEnvelope,
		message_sender: &StellarMessageSender,
	) -> Result<(), Error> {
		let slot = env.statement.slot_index;

		// set the last_slot_index
		self.set_last_slot_index(slot);

		// we are only interested with `ScpStExternalize`. Other messages are ignored.
		if let ScpStatementPledges::ScpStExternalize(stmt) = &env.statement.pledges {
			let txset_hash = get_tx_set_hash(stmt)?;

			// Check if collector has a record of this hash.
			if self.is_txset_new(&txset_hash, &slot) {
				// if it doesn't exist, let's request from the Stellar Node.
				tracing::debug!("requesting TxSet for slot {}...", slot);
				message_sender.send(StellarMessage::GetTxSet(txset_hash)).await?;

				// let's save this for creating the proof later on.
				self.save_txset_hash_and_slot(txset_hash, slot);
			}

			// insert/add the externalized message to map.
			self.add_scp_envelope(slot, env);
		} else {
			self.remove_data(&slot);
		}

		Ok(())
	}

	/// handles incoming TransactionSet.
	pub(crate) fn handle_tx_set(&self, set: TransactionSet) {
		// compute the tx_set_hash, to check what slot this set belongs too.
		let tx_set_hash = compute_non_generic_tx_set_content_hash(&set);

		// save this txset.
		self.add_txset(&tx_set_hash, set);
	}
}
