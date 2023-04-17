use std::{sync::Arc, time::Duration};

use service::on_shutdown;
use tokio::{
	sync::{mpsc, RwLock},
	time::{sleep, timeout},
};

use runtime::ShutdownSender;
use stellar_relay_lib::{
	connect_to_stellar_overlay_network, sdk::types::StellarMessage, StellarOverlayConfig,
	StellarRelayMessage,
};

use crate::oracle::{
	collector::ScpMessageCollector,
	errors::Error,
	types::{Slot, StellarMessageSender},
	Proof,
};

pub struct OracleAgent {
	collector: Arc<RwLock<ScpMessageCollector>>,
	pub is_public_network: bool,
	message_sender: Option<StellarMessageSender>,
	shutdown_sender: ShutdownSender,
}

/// listens to data to collect the scp messages and txsets.
/// # Arguments
///
/// * `message` - A message from the StellarRelay
/// * `collector` - used to collect envelopes and transaction sets
/// * `message_sender` - used to send messages to Stellar Node
async fn handle_message(
	message: StellarRelayMessage,
	collector: Arc<RwLock<ScpMessageCollector>>,
	message_sender: &StellarMessageSender,
) -> Result<(), Error> {
	match message {
		StellarRelayMessage::Data { p_id: _, msg_type: _, msg } => match *msg {
			StellarMessage::ScpMessage(env) => {
				collector.write().await.handle_envelope(env, message_sender).await?;
			},
			StellarMessage::TxSet(set) => {
				collector.read().await.handle_tx_set(set);
			},
			_ => {},
		},
		StellarRelayMessage::Connect { pub_key, node_info } => {
			tracing::info!("Connected: {:#?}\n via public key: {:?}", node_info, pub_key);
		},
		StellarRelayMessage::Timeout => {
			tracing::error!("The Stellar Relay timed out.");
		},
		_ => {},
	}

	Ok(())
}

/// Start the connection to the Stellar Node.
/// Returns an `OracleAgent` that will handle incoming messages from Stellar Node,
/// and to send messages to Stellar Node
pub async fn start_oracle_agent(
	config: StellarOverlayConfig,
	secret_key: &str,
) -> Result<OracleAgent, Error> {
	let mut overlay_conn = connect_to_stellar_overlay_network(config.clone(), secret_key).await?;

	// Get action sender and disconnect action before moving `overlay_conn` into the closure
	let actions_sender = overlay_conn.get_actions_sender();
	let disconnect_action = overlay_conn.get_disconnect_action();

	let (sender, mut receiver) = mpsc::channel(34);
	let collector = Arc::new(RwLock::new(ScpMessageCollector::new(
		config.is_public_network(),
		config.stellar_history_base_url(),
	)));
	let shutdown_sender = ShutdownSender::default();

	let shutdown_clone = shutdown_sender.clone();
	// handle a message from the overlay network
	let sender_clone = sender.clone();

	let collector_clone = collector.clone();
	service::spawn_cancelable(shutdown_clone.subscribe(), async move {
		let sender = sender_clone.clone();
		loop {
			tokio::select! {
				// runs the stellar-relay and listens to data to collect the scp messages and txsets.
				Some(msg) = overlay_conn.listen() => {
					handle_message(msg, collector_clone.clone(), &sender).await?;
				},

				Some(msg) = receiver.recv() => {
					// We received the instruction to send a message to the overlay network by the receiver
					overlay_conn.send(msg).await?;
				}
			}
		}
		#[allow(unreachable_code)]
		Ok::<(), Error>(())
	});

	tokio::spawn(on_shutdown(shutdown_sender.clone(), async move {
		let result_sending_diconnect =
			actions_sender.send(disconnect_action).await.map_err(Error::from);
		if let Err(e) = result_sending_diconnect {
			tracing::error!("Failed to send message to error : {:#?}", e);
		};
	}));

	Ok(OracleAgent {
		collector,
		is_public_network: false,
		message_sender: Some(sender),
		shutdown_sender,
	})
}

impl OracleAgent {
	/// This method returns the proof for a given slot or an error if the proof cannot be provided.
	/// The agent will try every possible way to get the proof before returning an error.
	/// Set timeout to 60 seconds; 10 seconds interval.
	pub async fn get_proof(&self, slot: Slot) -> Result<Proof, Error> {
		let sender = self
			.message_sender
			.clone()
			.ok_or_else(|| Error::Uninitialized("MessageSender".to_string()))?;

		let collector = self.collector.clone();

		timeout(Duration::from_secs(60), async move {
			loop {
				let stellar_sender = sender.clone();
				let collector = collector.read().await;
				match collector.build_proof(slot, &stellar_sender).await {
					None => {
						drop(collector);
						sleep(Duration::from_secs(10)).await;
						continue
					},
					Some(proof) => return Ok(proof),
				}
			}
		})
		.await
		.map_err(|elapsed| {
			Error::ProofTimeout(format!("Timeout elapsed for building proof: {:?}", elapsed))
		})?
	}

	pub async fn last_slot_index(&self) -> Slot {
		self.collector.read().await.last_slot_index()
	}

	pub async fn remove_data(&self, slot: &Slot) {
		self.collector.read().await.remove_data(slot);
	}

	/// Stops listening for new SCP messages.
	pub fn stop(&self) -> Result<(), Error> {
		tracing::info!("Stopping agent");
		if let Err(e) = self.shutdown_sender.send(()) {
			tracing::error!("Failed to send shutdown signal to the agent: {:?}", e);
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {

	use crate::oracle::{
		get_test_secret_key, get_test_stellar_relay_config, traits::ArchiveStorage,
		ScpArchiveStorage, TransactionsArchiveStorage,
	};

	use super::*;

	#[tokio::test]
	#[ntest::timeout(1_800_000)] // timeout at 30 minutes
	async fn test_get_proof_for_current_slot() {
		let agent =
			start_oracle_agent(get_test_stellar_relay_config(false), &get_test_secret_key(false))
				.await
				.expect("Failed to start agent");
		sleep(Duration::from_secs(10)).await;
		// Wait until agent is caught up with the network.

		let mut latest_slot = 0;
		while latest_slot == 0 {
			sleep(Duration::from_secs(1)).await;
			latest_slot = agent.last_slot_index().await;
		}
		// use the next slot to prevent receiving not enough messages for the current slot
		// because of bad timing when connecting to the network.
		latest_slot += 1;

		let proof_result = agent.get_proof(latest_slot).await;
		assert!(proof_result.is_ok(), "Failed to get proof for slot: {}", latest_slot);
	}

	#[tokio::test]
	async fn test_get_proof_for_archived_slot() {
		let scp_archive_storage = ScpArchiveStorage::default();
		let tx_archive_storage = TransactionsArchiveStorage::default();

		let agent =
			start_oracle_agent(get_test_stellar_relay_config(true), &get_test_secret_key(true))
				.await
				.expect("Failed to start agent");

		// This slot should be archived on the public network
		let target_slot = 44041116;
		let proof = agent.get_proof(target_slot).await.expect("should return a proof");

		assert_eq!(proof.slot(), 44041116);

		scp_archive_storage.remove_file(target_slot);
		tx_archive_storage.remove_file(target_slot);

		agent.stop().expect("Failed to stop the agent");
	}
}
