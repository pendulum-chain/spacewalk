use crate::oracle::{
	collector::{get_tx_set_hash, ScpMessageCollector},
	constants::{
		get_min_externalized_messages, MAX_DISTANCE_FROM_CURRENT_SLOT, MAX_SLOTS_PER_FILE,
	},
	errors::Error,
	storage::{traits::FileHandlerExt, EnvelopesFileHandler},
	types::{Slot, TxSetToSlotMap},
};
use parking_lot::RwLock;
use std::sync::Arc;
use stellar_relay::{
	sdk::types::{ScpEnvelope, ScpStatementPledges, StellarMessage},
	StellarOverlayConnection, xdr_converter
};
use stellar_relay::sdk::XdrCodec;
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
		txset_hash_map: &mut TxSetToSlotMap,
		overlay_conn: &StellarOverlayConnection,
	) -> Result<(), Error> {
		let slot = env.statement.slot_index;

		{
			let mut last_slot_index = self.last_slot_index_mut();
			if slot > *last_slot_index {
				*last_slot_index = slot;
			}
		}
		

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
				if value.len() < get_min_externalized_messages(self.public_network)
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
mod test {
	use crate::oracle::{
		collector::ScpMessageCollector,
		constants::MAX_SLOTS_PER_FILE,
		storage::{traits::FileHandler, EnvelopesFileHandler},
		types::Slot,
	};
	use mockall::lazy_static;
	use rand::Rng;
	use std::{collections::HashSet, convert::TryFrom};
	lazy_static! {
		static ref M_SLOTS_FILE: Slot =
			Slot::try_from(MAX_SLOTS_PER_FILE - 1).expect("should convert just fine");
	}

	#[test]
	fn check_write_envelopes_to_file_false() {
		let first_slot = 578291;
		let last_slot = first_slot + *M_SLOTS_FILE;

		let mut env_map = EnvelopesFileHandler::get_map_from_archives(first_slot + 3)
			.expect("should return a map");

		// let's remove some elements
		let mut rng = rand::thread_rng();
		let rand_num_of_elements = rng.gen_range(1..(*M_SLOTS_FILE / 2));
		let rand_num_of_elements =
			usize::try_from(rand_num_of_elements).expect("should convert ok");

		let res = rng
			.sample_iter(rand::distributions::Uniform::from(first_slot..last_slot))
			.take(rand_num_of_elements)
			.collect::<HashSet<Slot>>();

		res.iter().for_each(|slot| {
			assert!(env_map.remove(slot).is_some());
		});

		let mut collector = ScpMessageCollector::new(true, vec![]);
		collector.envelopes_map_mut().append(&mut env_map);

		// this should not write to file.
		let res = collector.check_write_envelopes_to_file(last_slot + 1).expect("should not fail");
		assert!(!res);
	}
}
