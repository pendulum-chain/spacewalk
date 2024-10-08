use std::{sync::Arc, time::Duration};

use tokio::{
	sync::RwLock,
	time::{sleep, timeout, Instant},
};

use runtime::ShutdownSender;
use stellar_relay_lib::{
	connect_to_stellar_overlay_network, helper::to_base64_xdr_string, sdk::types::StellarMessage,
	StellarOverlayConfig,
};

use crate::{
	oracle::{
		collector::ScpMessageCollector, errors::Error, types::StellarMessageSender, AddTxSet, Proof,
	},
	ArcRwLock,
};
use wallet::Slot;

pub struct OracleAgent {
	pub collector: Arc<RwLock<ScpMessageCollector>>,
	pub is_public_network: bool,
	/// sends message directly to Stellar Node
	message_sender: Option<StellarMessageSender>,
	/// sends an entire Vault shutdown
	shutdown_sender: ShutdownSender,
}

impl OracleAgent {
	pub fn new(config: &StellarOverlayConfig, shutdown_sender: ShutdownSender) -> Self {
		let is_public_network = config.is_public_network();

		let collector = Arc::new(RwLock::new(ScpMessageCollector::new(
			is_public_network,
			config.stellar_history_archive_urls(),
		)));

		OracleAgent { collector, is_public_network, message_sender: None, shutdown_sender }
	}

	#[cfg(any(test, feature = "integration"))]
	pub async fn is_stellar_running(&self) -> bool {
		self.message_sender.is_some() && self.collector.read().await.last_slot_index() > 0
	}
}

/// listens to data to collect the scp messages and txsets.
/// # Arguments
///
/// * `message` - A message from the StellarRelay
/// * `collector` - used to collect envelopes and transaction sets
/// * `message_sender` - used to send messages to Stellar Node
/// * `is_proof_building_ready` - set a signal to ready if a slot was saved
async fn handle_message(
	message: StellarMessage,
	collector: Arc<RwLock<ScpMessageCollector>>,
	message_sender: &StellarMessageSender,
	is_proof_building_ready: &mut Option<bool>,
) -> Result<(), Error> {
	match message {
		StellarMessage::ScpMessage(env) => {
			// if the first slot was saved, it means proof building is ready.
			if let Some(slot) = collector.write().await.handle_envelope(env, message_sender).await?
			{
				if let Some(true) = is_proof_building_ready {
					tracing::info!(
						"handle_message(): First slot saved: {slot}. Ready to build proofs "
					);
				}
				*is_proof_building_ready = None;
			}
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
	config: StellarOverlayConfig,
	oracle_agent: ArcRwLock<OracleAgent>,
	secret_key_as_string: String,
	shutdown_sender: ShutdownSender,
) -> Result<(), service::Error<crate::Error>> {
	tracing::info!(
		"listen_for_stellar_messages(): Starting connection to Stellar overlay network..."
	);

	let mut overlay_conn = connect_to_stellar_overlay_network(config.clone(), secret_key_as_string)
		.await.map_err(|e|{
			tracing::error!("listen_for_stellar_messages(): Failed to connect to Stellar overlay network: {e:?}");
			service::Error::StartOracleAgentError
		})?;

	// use StellarOverlayConnection's sender to send message to Stellar
	let sender = overlay_conn.sender();
	{
		oracle_agent.write().await.message_sender = Some(sender.clone());
	};

	// log a new message received, every 1 minute.
	let interval = Duration::from_secs(60);
	let mut next_time = Instant::now() + interval;
	let mut is_proof_building_ready: Option<bool> = Some(false);
	loop {
		let collector = oracle_agent.read().await.collector.clone();

		match overlay_conn.listen().await {
			Ok(None) => {},
			Ok(Some(StellarMessage::Hello(_))) => {
				tracing::info!(
					"listen_for_stellar_messages(): received hello message from Stellar"
				);
				is_proof_building_ready = Some(true);
			},
			Ok(Some(StellarMessage::ErrorMsg(e))) => {
				tracing::error!(
					"listen_for_stellar_messages(): received error message from Stellar: {e:?}"
				);
				break
			},
			Ok(Some(msg)) => {
				let msg_as_str = to_base64_xdr_string(&msg);
				if Instant::now() >= next_time {
					tracing::info!("listen_for_stellar_messages(): health check: received message from Stellar");
					next_time += interval;
				}

				if let Some(true) = is_proof_building_ready {
					tracing::info!("listen_for_stellar_messages(): received message from Stellar: {msg_as_str}");
				};

				if let Err(e) =
					handle_message(msg, collector.clone(), &sender, &mut is_proof_building_ready)
						.await
				{
					tracing::error!("listen_for_stellar_messages(): failed to handle message: {msg_as_str}: {e:?}");
				}
			},
			// connection got lost
			Err(e) => {
				tracing::error!("listen_for_stellar_messages(): encounter error in overlay: {e:?}");
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
	_vault_stellar_secret: String,
	shutdown_sender: ShutdownSender,
) -> ArcRwLock<OracleAgent> {
	let oracle_agent = Arc::new(RwLock::new(OracleAgent::new(&cfg, shutdown_sender.clone())));

	let secret = crate::oracle::get_random_secret_key();

	tokio::spawn(listen_for_stellar_messages(cfg, oracle_agent.clone(), secret, shutdown_sender));

	while !oracle_agent.read().await.is_stellar_running().await {
		sleep(Duration::from_millis(500)).await;
	}

	tracing::info!("start_oracle_agent(): Stellar overlay network is running");
	oracle_agent
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
				tracing::info!("get_proof(): attempt to build proof for slot {slot}");
				let stellar_sender = sender.clone();
				let collector = collector.read().await;
				match collector.build_proof(slot, &stellar_sender).await {
					None => {
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
			let slot = agent.read().await.collector.read().await.last_slot_index();
			if slot > 0 {
				break slot
			}
			sleep(Duration::from_millis(500)).await;
		};

		let proof_result = agent.read().await.get_proof(latest_slot).await;
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
		let proof = agent.read().await.get_proof(target_slot).await.expect("should return a proof");

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
		let proof = agent.read().await.get_proof(target_slot).await.expect("should return a proof");

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

		let proof_result = agent.read().await.get_proof(target_slot).await;

		assert!(matches!(proof_result, Err(Error::ProofTimeout(_))));

		// These might return an error if the file does not exist, but that's fine.
		let _ = scp_archive_storage.remove_file(target_slot);
		let _ = tx_archive_storage.remove_file(target_slot);
	}
}
