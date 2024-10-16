use crate::oracle::{
	collector::{get_tx_set_hash, ScpMessageCollector},
	errors::Error,
	types::StellarMessageSender,
};
use primitives::stellar::{
	types::{ScpEnvelope, ScpStatementPledges, StellarMessage},
	StellarTypeToBase64String,
};

// Handling SCPEnvelopes
impl ScpMessageCollector {
	/// Handles incoming ScpEnvelope. Return slot if it was saved
	///
	/// # Arguments
	///
	/// * `env` - the ScpEnvelope
	/// * `message_sender` - used for sending messages to Stellar Node
	pub(crate) async fn handle_envelope(
		&mut self,
		env: ScpEnvelope,
		message_sender: &StellarMessageSender,
	) -> Result<Option<u64>, Error> {
		let slot = env.statement.slot_index;

		// we are only interested with `ScpStExternalize`. Other messages are ignored.
		if let ScpStatementPledges::ScpStExternalize(stmt) = &env.statement.pledges {
			tracing::trace!(
				"Handling Incoming ScpEnvelopes for slot {slot}: SCPStExternalize found: {}",
				stmt.as_base64_encoded_string()
			);

			if self.last_slot_index() == 0 {
				tracing::info!("handle_envelope(): for slot {slot}: first SCPStExternalize found");
			}
			// set the last scpenvenvelope with ScpStExternalize message
			self.set_last_slot_index(slot);

			let txset_hash = get_tx_set_hash(stmt)?;

			// Check if collector has a record of this hash.
			if self.is_txset_new(&txset_hash, &slot) {
				// if it doesn't exist, let's request from the Stellar Node.
				tracing::debug!(
					"Handling Incoming ScpEnvelopes for slot {slot}: requesting TxSet..."
				);
				message_sender.send(StellarMessage::GetTxSet(txset_hash)).await?;

				// let's save this for creating the proof later on.
				self.save_txset_hash_and_slot(txset_hash, slot);
			}

			// insert/add the externalized message to map.
			self.add_scp_envelope(slot, env);
			Ok(Some(slot))
		} else {
			self.remove_data(&slot);
			Ok(None)
		}
	}
}
