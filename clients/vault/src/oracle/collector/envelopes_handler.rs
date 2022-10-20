use crate::oracle::{
	collector::{get_tx_set_hash, ScpMessageCollector},
	constants::{MAX_SLOTS_PER_FILE, MIN_EXTERNALIZED_MESSAGES},
	errors::Error,
	storage::{traits::FileHandlerExt, EnvelopesFileHandler},
	types::{Slot, TxSetCheckerMap},
};
use parking_lot::RwLock;
use std::sync::Arc;
use stellar_relay::{
	sdk::types::{ScpEnvelope, ScpStatementPledges, StellarMessage},
	StellarOverlayConnection,
};

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
				tracing::info!("Adding received SCP envelopes for slot {}", slot);
				envelopes_map.insert(slot, vec![env]);
			}
		}

		Ok(())
	}

	/// Checks whether the envelopes map requires saving to file.
	/// One of the factors would be "how old" the slot is, against the current slot.
	fn check_write_envelopes_to_file(&mut self, current_slot: Slot) -> Result<bool, Error> {
		let env_map = self.envelopes_map().clone();
		let mut slots = env_map.keys();
		let slots_len = slots.len();

		// map is too small; we don't have to write it to file just yet.
		if slots_len < MAX_SLOTS_PER_FILE {
			return Ok(false)
		}

		tracing::debug!("Size of envelopes map exceeds limit. Writing it to file...");

		let mut counter = 0;
		while let Some(slot) = slots.next() {
			// ends the loop and saves the map to a file.
			if counter == MAX_SLOTS_PER_FILE {
				self.write_envelopes_to_file(*slot)?;
				return Ok(true)
			}

			if let Some(value) = env_map.get(slot) {
				// check if we have enough externalized messages for the corresponding key
				if value.len() < MIN_EXTERNALIZED_MESSAGES
                    // if the key is too far back from the current slot,
                    // then we don't need to wait for more messages.
                    && (current_slot - *slot) < MAX_DISTANCE_FROM_CURRENT_SLOT
				{
					tracing::warn!("slot: {} not enough messages. Let's wait for more.", slot);
					break
				}
			} else {
				tracing::error!("something wrong??? race condition?");
				// something wrong??? race condition?
				break
			}

			counter += 1;
		}

		Ok(false)
	}

	/// saves a portion/or everything of the `envelopes_map` to a file.
	fn write_envelopes_to_file(&mut self, last_slot: Slot) -> Result<(), Error> {
		let new_slot_map = self.envelopes_map_mut().split_off(&last_slot);
		let res = EnvelopesFileHandler::write_to_file(&self.envelopes_map())?;
		tracing::info!("writing old envelopes map to file: {}", res);
		self.envelopes_map = Arc::new(RwLock::new(new_slot_map));
		tracing::debug!("Oldest slot # is: {:?}", last_slot);

		Ok(())
	}
}

#[cfg(test)]
#[path = "envelopes_handler_tests.rs"]
mod envelopes_handler_tests;
