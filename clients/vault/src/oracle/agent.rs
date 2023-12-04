use std::{sync::Arc, time::Duration};

use service::on_shutdown;
use tokio::{
	sync::{mpsc, RwLock},
	time::{sleep, timeout},
};
use tokio::sync::Mutex;
use tokio::time::error::Elapsed;
use tokio::time::Timeout;
use tracing::log;

use runtime::ShutdownSender;
use stellar_relay_lib::{connect_to_stellar_overlay_network, sdk::types::StellarMessage, StellarOverlayConfig, StellarOverlayConnection};
use stellar_relay_lib::helper::to_base64_xdr_string;

use crate::oracle::{
	collector::ScpMessageCollector,
	errors::Error,
	types::{Slot, StellarMessageSender},
	AddTxSet, Proof,
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
	message: StellarMessage,
	collector: Arc<RwLock<ScpMessageCollector>>,
	message_sender: &StellarMessageSender,
) -> Result<(), Error> {
	match message {
		StellarMessage::ScpMessage(env) => {
			collector.write().await.handle_envelope(env, message_sender).await?;
		},
		StellarMessage::TxSet(set) =>
			if let Err(e) = collector.read().await.add_txset(set) {
				tracing::error!(e);
			},
		StellarMessage::GeneralizedTxSet(set) => {
			if let Err(e) = collector.read().await.add_txset(set) {
				tracing::error!(e);
			}
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
	let timeout_in_secs = config.connection_info.timeout_in_secs;
	let secret_key_copy = secret_key.to_string();

	tracing::info!("start_oracle_agent(): Starting connection to Stellar overlay network...");

	let mut overlay_conn = connect_to_stellar_overlay_network(config.clone(), secret_key).await?;

	let collector = Arc::new(RwLock::new(ScpMessageCollector::new(
		config.is_public_network(),
		config.stellar_history_archive_urls(),
	)));
	let collector_clone = collector.clone();

	let shutdown_sender = ShutdownSender::default();
	let shutdown_sender_clone = shutdown_sender.clone();
	let shutdown_sender_clone2 = shutdown_sender.clone();

	// disconnect signal sender
	let (disconnect_signal_sender, mut disconnect_signal_receiver) = mpsc::channel::<()>(2);

	// handle a message from the overlay network
	let (message_sender, mut message_receiver) = mpsc::channel::<StellarMessage>(34);
	let message_sender_clone = message_sender.clone();

	let mut reconnection_increment = 0;
	service::spawn_cancelable(shutdown_sender_clone.subscribe(), async move {
		let mut stop_overlay = false;
		loop {
			if !overlay_conn.is_alive() {
				tracing::info!("start_oracle_agent(): oracle is dead.");
				let _ = shutdown_sender_clone2.send(());

				message_receiver.close();
				disconnect_signal_receiver.close();

				return Ok(());
			}

			tokio::select! {
				result_msg = overlay_conn.listen() => { match result_msg {
					Ok(Some(msg)) => {
						//tracing::info!("start_oracle_agent(): handle message: {}", to_base64_xdr_string(&msg));
						handle_message(
							msg,
							collector_clone.clone(),
							&message_sender_clone
						).await?;
					}
					Ok(None) => {}
					Err(e) => {
						tracing::error!("start_oracle_agent(): received error: {e:?}");

						overlay_conn.disconnect();
						let _ = shutdown_sender_clone2.send(());
						return Ok(());
						// loop {
						// 	let sleep_time = timeout_in_secs + reconnection_increment;
						// 	tracing::info!("start_oracle_agent(): reconnect to Stellar in {sleep_time} seconds");
						// 	reconnection_increment += 5; // increment by 5
						//
						// 	sleep(Duration::from_secs(timeout_in_secs + reconnection_increment)).await;
						//
						// 	match connect_to_stellar_overlay_network(config.clone(), &secret_key_copy).await {
						// 		Ok(new_overlay_conn) => {
						// 			overlay_conn = new_overlay_conn;
						//
						// 			// wait to be sure that a connection was established.
						// 			sleep(Duration::from_secs(timeout_in_secs)).await;
						//
						// 			if overlay_conn.is_alive() {
						// 				tracing::info!("start_oracle_agent(): new connection created...");
						// 				// a new connection was created; break out of this inner loop
						// 				// and renew listening for messages
						// 				break;
						// 			}
						// 		}
						// 		Err(e) => tracing::warn!("start_oracle_agent(): reconnection failed: {e:?}")
						// 	}
						// }
					}
				}},

				result_msg = timeout(Duration::from_secs(10), message_receiver.recv()) => match result_msg {
					Ok(Some(msg)) => if let Err(e) = overlay_conn.send_to_node(msg).await {
						tracing::error!("start_oracle_agent(): failed to send msg to stellar node: {e:?}");
					},
					_ => {}
				},

				result_msg = timeout(Duration::from_secs(10), disconnect_signal_receiver.recv()) => match result_msg {
					Ok(Some(_)) => {
						tracing::info!("start_oracle_agent(): disconnect signal received.");

						overlay_conn.disconnect();
						let _ = shutdown_sender_clone2.send(());
						return Ok(());
					}
					_ => {}
				}
			}
		}

		tracing::info!("start_oracle_agent(): LOOP STOPPED!");

		Ok::<(), Error>(())
	});


	tokio::spawn(on_shutdown(shutdown_sender.clone(), async move {
		tracing::info!("start_oracle_agent(): sending signal to shutdown overlay connection...");
		let _ = disconnect_signal_sender.send(()).await;
	}));

	Ok(OracleAgent {
		collector,
		is_public_network: false,
		message_sender: Some(message_sender),
		shutdown_sender,
	})
}

impl OracleAgent {
	/// This method returns the proof for a given slot or an error if the proof cannot be provided.
	/// The agent will try every possible way to get the proof before returning an error.
	pub async fn get_proof(&self, slot: Slot) -> Result<Proof, Error> {
		let sender = self
			.message_sender
			.clone()
			.ok_or_else(|| Error::Uninitialized("MessageSender".to_string()))?;

		let collector = self.collector.clone();

		#[cfg(test)]
		let timeout_seconds = 180;

		#[cfg(not(test))]
		let timeout_seconds = 60;

		timeout(Duration::from_secs(timeout_seconds), async move {
			loop {
				let stellar_sender = sender.clone();
				let collector = collector.read().await;
				match collector.build_proof(slot, &stellar_sender).await {
					None => {
						tracing::warn!("get_proof(): Failed to build proof for slot {slot}.");
						drop(collector);
						// give 10 seconds interval for every retry
						sleep(Duration::from_secs(10)).await;
						continue
					},
					Some(proof) => {
						tracing::info!("get_proof(): Successfully build proof for slot {slot}");
						tracing::trace!("  with proof: {proof:?}");
						return Ok(proof)
					},
				}
			}
		})
		.await
		.map_err(|_| {
			Error::ProofTimeout(format!("Timeout elapsed for building proof of slot {slot}"))
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
		tracing::debug!("stop(): Shutting down OracleAgent...");
		if let Err(e) = self.shutdown_sender.send(()) {
			tracing::error!("stop(): Failed to send shutdown signal in OracleAgent: {:?}", e);
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
	use serial_test::serial;

	#[tokio::test]
	#[ntest::timeout(1_800_000)] // timeout at 30 minutes
	#[serial]
	async fn test_get_proof_for_current_slot() {
		let agent =
			start_oracle_agent(get_test_stellar_relay_config(true), &get_test_secret_key(true))
				.await
				.expect("Failed to start agent");
		sleep(Duration::from_secs(10)).await;
		// Wait until agent is caught up with the network.

		let mut latest_slot = 0;
		while latest_slot == 0 {
			sleep(Duration::from_secs(1)).await;
			latest_slot = agent.last_slot_index().await;
		}
		// use a future slot (2 slots ahead) to ensure enough messages can be collected
		// and to avoid "missed" messages.
		latest_slot += 2;

		let proof_result = agent.get_proof(latest_slot).await;
		assert!(proof_result.is_ok(), "Failed to get proof for slot: {}", latest_slot);
	}

	#[tokio::test]
	#[serial]
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

		// These might return an error if the file does not exist, but that's fine.
		let _ = scp_archive_storage.remove_file(target_slot);
		let _ = tx_archive_storage.remove_file(target_slot);

		agent.stop().expect("Failed to stop the agent");
	}

	#[tokio::test]
	#[serial]
	async fn test_get_proof_for_archived_slot_with_fallback() {
		let scp_archive_storage = ScpArchiveStorage::default();
		let tx_archive_storage = TransactionsArchiveStorage::default();

		let base_config = get_test_stellar_relay_config(true);
		// We add two fake archive urls to the config to make sure that the agent will actually fall
		// back to other archives.
		let mut archive_urls = base_config.stellar_history_archive_urls().clone();
		archive_urls.push("https://my-fake-archive.org".to_string());
		archive_urls.push("https://my-fake-archive-2.org".to_string());
		archive_urls.reverse();
		let modified_config =
			StellarOverlayConfig { stellar_history_archive_urls: archive_urls, ..base_config };

		let agent = start_oracle_agent(modified_config, &get_test_secret_key(true))
			.await
			.expect("Failed to start agent");

		// This slot should be archived on the public network
		let target_slot = 44041116;
		let proof = agent.get_proof(target_slot).await.expect("should return a proof");

		assert_eq!(proof.slot(), 44041116);

		// These might return an error if the file does not exist, but that's fine.
		let _ = scp_archive_storage.remove_file(target_slot);
		let _ = tx_archive_storage.remove_file(target_slot);

		agent.stop().expect("Failed to stop the agent");
	}

	#[tokio::test]
	#[serial]
	async fn test_get_proof_for_archived_slot_fails_without_archives() {
		let scp_archive_storage = ScpArchiveStorage::default();
		let tx_archive_storage = TransactionsArchiveStorage::default();

		let base_config = get_test_stellar_relay_config(true);
		let modified_config: StellarOverlayConfig =
			StellarOverlayConfig { stellar_history_archive_urls: vec![], ..base_config };

		let agent = start_oracle_agent(modified_config, &get_test_secret_key(true))
			.await
			.expect("Failed to start agent");

		// This slot should be archived on the public network
		let target_slot = 44041116;
		let proof_result = agent.get_proof(target_slot).await;

		assert!(matches!(proof_result, Err(Error::ProofTimeout(_))));

		// These might return an error if the file does not exist, but that's fine.
		let _ = scp_archive_storage.remove_file(target_slot);
		let _ = tx_archive_storage.remove_file(target_slot);

		agent.stop().expect("Failed to stop the agent");
	}
}
