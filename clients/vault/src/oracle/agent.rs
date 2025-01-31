use std::{sync::Arc, time::Duration};

use primitives::stellar::StellarTypeToBase64String;
use tokio::{
	sync::RwLock,
	time::{sleep, timeout, Instant},
};
use tracing::error;
use runtime::ShutdownSender;
use stellar_relay_lib::{
	connect_to_stellar_overlay_network, sdk::types::StellarMessage, StellarOverlayConfig,
	StellarOverlayConnection,
};

use crate::{
	oracle::{
		collector::ScpMessageCollector, errors::Error, types::StellarMessageSender, AddTxSet, Proof,
	},
	ArcRwLock,
};
use wallet::Slot;

/// The interval to check if we are still receiving messages from Stellar Relay
const STELLAR_RELAY_HEALTH_CHECK_IN_SECS: u64 = 300;
/// The waiting time for reading messages from the overlay.
static STELLAR_MESSAGES_TIMEOUT_IN_SECS: u64 = 60;

pub struct OracleAgent {
	pub collector: ArcRwLock<ScpMessageCollector>,
	pub is_public_network: bool,
	/// sends message directly to Stellar Node
	message_sender: StellarMessageSender,
	overlay_conn: ArcRwLock<StellarOverlayConnection>,
	/// sends an entire Vault shutdown
	shutdown_sender: ShutdownSender,
}

impl OracleAgent {
	// the interval for every build_proof retry
	const BUILD_PROOF_INTERVAL: u64 = 10;

	pub async fn new(
		config: &StellarOverlayConfig,
		secret_key_as_string: String,
		shutdown_sender: ShutdownSender,
	) -> Result<Self, Error> {
		let is_public_network = config.is_public_network();

		let collector = Arc::new(RwLock::new(ScpMessageCollector::new(
			is_public_network,
			config.stellar_history_archive_urls(),
		)));

		let overlay_conn =
			connect_to_stellar_overlay_network(config.clone(), secret_key_as_string).await?;
		let message_sender = overlay_conn.sender();

		let overlay_conn = Arc::new(RwLock::new(overlay_conn));

		Ok(OracleAgent {
			collector,
			is_public_network,
			message_sender,
			overlay_conn,
			shutdown_sender,
		})
	}

	/// This method returns the proof for a given slot or an error if the proof cannot be provided.
	/// The agent will try every possible way to get the proof before returning an error.
	pub async fn get_proof(&self, slot: Slot) -> Result<Proof, Error> {
		let collector = self.collector.clone();

		#[cfg(test)]
		let timeout_seconds = 180;

		#[cfg(not(test))]
		let timeout_seconds = 60;

		timeout(Duration::from_secs(timeout_seconds), async move {
			loop {
				tracing::debug!("get_proof(): attempt to build proof for slot {slot}");
				let collector = collector.read().await;
				match collector.build_proof(slot, &self.message_sender).await {
					None => {
						drop(collector);
						// give enough interval for every retry
						sleep(Duration::from_secs(OracleAgent::BUILD_PROOF_INTERVAL)).await;
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

	#[cfg(any(test, feature = "integration"))]
	pub async fn is_stellar_running(&self) -> bool {
		self.collector.read().await.last_slot_index() > 0
	}
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

pub async fn listen_for_stellar_messages(
	oracle_agent: Arc<OracleAgent>,
	shutdown_sender: ShutdownSender,
) -> Result<(), service::Error<crate::Error>> {
	tracing::info!("listen_for_stellar_messages(): started");

	let mut overlay_conn = oracle_agent.overlay_conn.write().await;

	// log a new message received.
	let health_check_interval = Duration::from_secs(STELLAR_RELAY_HEALTH_CHECK_IN_SECS);

	let mut next_time = Instant::now() + health_check_interval;
	let mut last_valid_message_time = Instant::now();
	loop {
		let collector = oracle_agent.collector.clone();
		if last_valid_message_time < (Instant::now() - health_check_interval) { break }

		match timeout(
			Duration::from_secs(STELLAR_MESSAGES_TIMEOUT_IN_SECS),
			overlay_conn.listen(),
		).await {
			Ok(Ok(None)) => {},
			Ok(Ok(Some(StellarMessage::ErrorMsg(e)))) => {
				tracing::error!(
					"listen_for_stellar_messages(): received error message from Stellar: {e:?}"
				);
				break
			},
			Ok(Ok(Some(msg))) => {
				last_valid_message_time = Instant::now();
				if Instant::now() >= next_time {
					tracing::info!("listen_for_stellar_messages(): health check: received message from Stellar");
					next_time += health_check_interval;
				}

				if let Err(e) =
					handle_message(msg.clone(), collector.clone(), &oracle_agent.message_sender)
						.await
				{
					let msg_as_str = msg.as_base64_encoded_string();
					tracing::error!("listen_for_stellar_messages(): failed to handle message: {msg_as_str}: {e:?}");
				}
			},
			// connection got lost
			Ok(Err(e)) => {
				tracing::error!("listen_for_stellar_messages(): encounter error in overlay: {e:?}");
				break
			},
			Err(_) => {
				error!("listen_for_stellar_messages(): overlay_conn.listen() timed out");
				break
			},
		}
	}

	if let Err(e) = shutdown_sender.send(()) {
		tracing::error!(
			"listen_for_stellar_messages(): Failed to send shutdown signal in thread: {e:?}"
		);
	}

	tracing::info!("listen_for_stellar_messages(): shutting down overlay connection");
	overlay_conn.stop();

	Ok(())
}
#[cfg(any(test, feature = "integration"))]
pub async fn start_oracle_agent(
	cfg: StellarOverlayConfig,
	vault_stellar_secret: String,
	shutdown_sender: ShutdownSender,
) -> Arc<OracleAgent> {
	let oracle_agent = Arc::new(
		OracleAgent::new(&cfg, vault_stellar_secret, shutdown_sender.clone())
			.await
			.expect("should work"),
	);

	tokio::spawn(listen_for_stellar_messages(oracle_agent.clone(), shutdown_sender));

	while !oracle_agent.is_stellar_running().await {
		sleep(Duration::from_millis(500)).await;
	}

	tracing::info!("start_oracle_agent(): Stellar overlay network is running");
	oracle_agent
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::oracle::{
		get_random_secret_key, specific_stellar_relay_config, traits::ArchiveStorage,
		ScpArchiveStorage, TransactionsArchiveStorage,
	};
	use serial_test::serial;
	use wallet::keys::get_source_secret_key_from_env;

	#[tokio::test(flavor = "multi_thread")]
	#[ntest::timeout(600_000)] // timeout at 10 minutes
	#[serial]
	async fn test_get_proof_for_current_slot() {
		// let it run for a few seconds, making sure that the other tests have successfully shutdown
		// their connection to Stellar Node
		sleep(Duration::from_secs(2)).await;

		let shutdown_sender = ShutdownSender::new();

		// We use a random secret key to avoid conflicts with other tests.
		let agent = start_oracle_agent(
			specific_stellar_relay_config(true, 0),
			get_random_secret_key(),
			shutdown_sender,
		)
		.await;

		let latest_slot = loop {
			let slot = agent.collector.read().await.last_slot_index();
			if slot > 0 {
				break slot
			}
			sleep(Duration::from_millis(500)).await;
		};

		let proof_result = agent.get_proof(latest_slot).await;
		assert!(proof_result.is_ok(), "Failed to get proof for slot: {}", latest_slot);
	}

	#[tokio::test(flavor = "multi_thread")]
	#[serial]
	async fn test_get_proof_for_archived_slot() {
		// let it run for a few seconds, making sure that the other tests have successfully shutdown
		// their connection to Stellar Node
		sleep(Duration::from_secs(2)).await;
		let is_public_network = true;
		let scp_archive_storage = ScpArchiveStorage::default();
		let tx_archive_storage = TransactionsArchiveStorage::default();

		let shutdown_sender = ShutdownSender::new();
		let agent = start_oracle_agent(
			specific_stellar_relay_config(is_public_network, 1),
			get_source_secret_key_from_env(is_public_network),
			shutdown_sender,
		)
		.await;

		// This slot should be archived on the public network
		let target_slot = 44041116;
		let proof = agent.get_proof(target_slot).await.expect("should return a proof");

		assert_eq!(proof.slot(), 44041116);

		// These might return an error if the file does not exist, but that's fine.
		let _ = scp_archive_storage.remove_file(target_slot);
		let _ = tx_archive_storage.remove_file(target_slot);
	}

	#[tokio::test(flavor = "multi_thread")]
	#[serial]
	async fn test_get_proof_for_archived_slot_with_fallback() {
		// let it run for a few seconds, making sure that the other tests have successfully shutdown
		// their connection to Stellar Node
		sleep(Duration::from_secs(2)).await;
		let is_public_network = true;
		let scp_archive_storage = ScpArchiveStorage::default();
		let tx_archive_storage = TransactionsArchiveStorage::default();

		let base_config = specific_stellar_relay_config(true, 2);
		// We add two fake archive urls to the config to make sure that the agent will actually fall
		// back to other archives.
		let mut archive_urls = base_config.stellar_history_archive_urls().clone();
		archive_urls.push("https://my-fake-archive.org".to_string());
		archive_urls.push("https://my-fake-archive-2.org".to_string());
		archive_urls.reverse();
		let modified_config =
			StellarOverlayConfig { stellar_history_archive_urls: archive_urls, ..base_config };

		let shutdown_sender = ShutdownSender::new();
		let agent = start_oracle_agent(
			modified_config,
			get_source_secret_key_from_env(is_public_network),
			shutdown_sender,
		)
		.await;

		// This slot should be archived on the public network
		let target_slot = 44041116;
		let proof = agent.get_proof(target_slot).await.expect("should return a proof");

		assert_eq!(proof.slot(), 44041116);

		// These might return an error if the file does not exist, but that's fine.
		let _ = scp_archive_storage.remove_file(target_slot);
		let _ = tx_archive_storage.remove_file(target_slot);
	}

	#[tokio::test(flavor = "multi_thread")]
	#[serial]
	async fn test_get_proof_for_archived_slot_fails_without_archives() {
		let scp_archive_storage = ScpArchiveStorage::default();
		let tx_archive_storage = TransactionsArchiveStorage::default();
		let is_public_network = true;

		let base_config = specific_stellar_relay_config(true, 0);
		let modified_config: StellarOverlayConfig =
			StellarOverlayConfig { stellar_history_archive_urls: vec![], ..base_config };

		let shutdown = ShutdownSender::new();
		let agent = start_oracle_agent(
			modified_config,
			get_source_secret_key_from_env(is_public_network),
			shutdown,
		)
		.await;

		// This slot should be archived on the public network
		let target_slot = 44041116;

		let proof_result = agent.get_proof(target_slot).await;

		assert!(matches!(proof_result, Err(Error::ProofTimeout(_))));

		// These might return an error if the file does not exist, but that's fine.
		let _ = scp_archive_storage.remove_file(target_slot);
		let _ = tx_archive_storage.remove_file(target_slot);
	}
}
