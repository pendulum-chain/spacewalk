use crate::oracle::{
	collector::{get_tx_set_hash, ScpMessageCollector},
	errors::Error,
};
use stellar_relay_lib::{
	helper::compute_non_generic_tx_set_content_hash,
	sdk::types::{ScpEnvelope, ScpStatementPledges, StellarMessage, TransactionSet},
	StellarOverlayConnection,
};

// Handling SCPEnvelopes
impl ScpMessageCollector {
	/// handles incoming ScpEnvelope.
	///
	/// # Arguments
	///
	/// * `env` - the ScpEnvelope
	/// * `overlay_conn` - The StellarOverlayConnection used for sending messages to Stellar Node
	pub(crate) async fn handle_envelope(
		&mut self,
		env: ScpEnvelope,
		overlay_conn: &StellarOverlayConnection,
	) -> Result<(), Error> {
		let slot = env.statement.slot_index;

		// tracing::info!("received envelope for slot {}", slot);

		// set the last_slot_index
		self.set_last_slot_index(slot);

		// ignore if slot is not in the list
		if self.is_slot_relevant(&slot) {
			tracing::trace!("found envelope for slot {}", slot);
		}

		// we are only interested with `ScpStExternalize`. Other messages are ignored.
		if let ScpStatementPledges::ScpStExternalize(stmt) = &env.statement.pledges {
			let txset_hash = get_tx_set_hash(stmt)?;

			// Check if collector has a record of this hash.
			if self.is_txset_new(&txset_hash, &slot) {
				// if it doesn't exist, let's request from the Stellar Node.
				tracing::debug!("requesting TxSet for slot {}...", slot);
				overlay_conn.send(StellarMessage::GetTxSet(txset_hash)).await?;

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
	pub(crate) fn handle_tx_set(&mut self, set: TransactionSet) {
		// compute the tx_set_hash, to check what slot this set belongs too.
		let tx_set_hash = compute_non_generic_tx_set_content_hash(&set);

		// save this txset.
		self.add_txset(&tx_set_hash, set);
	}
}

// #[cfg(test)]
// mod test {
// 	use crate::oracle::{
// 		collector::ScpMessageCollector,
// 		constants::MAX_SLOTS_PER_FILE,
// 		storage::{traits::FileHandler, EnvelopesFileHandler},
// 		types::Slot,
// 	};
// 	use mockall::lazy_static;
// 	use rand::Rng;
// 	use std::{collections::HashSet, convert::TryFrom};
// 	use tokio::sync::mpsc;
// 	lazy_static! {
// 		static ref M_SLOTS_FILE: Slot =
// 			Slot::try_from(MAX_SLOTS_PER_FILE - 1).expect("should convert just fine");
// 	}
//
// 	#[test]
// 	fn check_write_envelopes_to_file_false() {
// 		let first_slot = 578291;
// 		let last_slot = first_slot + *M_SLOTS_FILE;
//
// 		let mut env_map = EnvelopesFileHandler::get_map_from_archives(first_slot + 3)
// 			.expect("should return a map");
//
// 		// let's remove some elements
// 		let mut rng = rand::thread_rng();
// 		let rand_num_of_elements = rng.gen_range(1..(*M_SLOTS_FILE / 2));
// 		let rand_num_of_elements =
// 			usize::try_from(rand_num_of_elements).expect("should convert ok");
//
// 		let res = rng
// 			.sample_iter(rand::distributions::Uniform::from(first_slot..last_slot))
// 			.take(rand_num_of_elements)
// 			.collect::<HashSet<Slot>>();
//
// 		res.iter().for_each(|slot| {
// 			assert!(env_map.remove(slot).is_some());
// 		});
//
// 		let (sender, _) = mpsc::channel(1024);
// 		let mut collector = ScpMessageCollector::new(true, vec![]);
// 		collector.envelopes_map_mut().append(&mut env_map);
//
// 		// this should not write to file.
// 		let res = collector.check_write_envelopes_to_file(last_slot + 1).expect("should not fail");
// 		assert!(!res);
// 	}
// }
