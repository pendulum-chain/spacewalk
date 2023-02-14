use std::{sync::Arc, time::Duration};

use rand::RngCore;
use service::on_shutdown;
use tokio::{
	sync::mpsc,
	time::{sleep, timeout},
};

use runtime::ShutdownSender;
use stellar_relay_lib::{
	node::NodeInfo,
	sdk::{
		network::{PUBLIC_NETWORK, TEST_NETWORK},
		types::StellarMessage,
		SecretKey,
	},
	ConnConfig, StellarOverlayConnection, StellarRelayMessage,
};

use crate::oracle::{
	collector::ScpMessageCollector,
	constants::*,
	errors::Error,
	types::{Slot, StellarMessageSender},
	Proof,
};

pub struct OracleAgent {
	collector: Arc<ScpMessageCollector>,
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
	collector: &Arc<ScpMessageCollector>,
	message_sender: &StellarMessageSender,
) -> Result<(), Error> {
	match message {
		StellarRelayMessage::Data { p_id: _, msg_type: _, msg } => match *msg {
			StellarMessage::ScpMessage(env) => {
				collector.handle_envelope(env, message_sender).await?;
			},
			StellarMessage::TxSet(set) => {
				collector.handle_tx_set(set);
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

impl OracleAgent {
	pub fn new(is_public_network: bool) -> Result<Self, Error> {
		let collector = Arc::new(ScpMessageCollector::new(is_public_network));
		let shutdown_sender = ShutdownSender::default();
		Ok(Self { collector, is_public_network, message_sender: None, shutdown_sender })
	}

	fn get_overlay_conn_config(&self, secret_key: SecretKey) -> (NodeInfo, ConnConfig) {
		if self.is_public_network {
			let network = &PUBLIC_NETWORK;
			let node_info = NodeInfo::new(
				LEDGER_VERSION_PUBNET,
				OVERLAY_VERSION_PUBNET,
				MIN_OVERLAY_VERSION_PUBNET,
				VERSION_STRING_PUBNET.to_string(),
				network,
			);

			let cfg = ConnConfig::new(
				TIER_1_NODE_IP_PUBNET,
				TIER_1_NODE_PORT_PUBNET,
				secret_key,
				0,
				true,
				true,
				false,
			);
			(node_info, cfg)
		} else {
			let network = &TEST_NETWORK;
			let node_info = NodeInfo::new(
				LEDGER_VERSION_TESTNET,
				OVERLAY_VERSION_TESTNET,
				MIN_OVERLAY_VERSION_TESTNET,
				VERSION_STRING_TESTNET.to_string(),
				network,
			);

			let cfg = ConnConfig::new(
				TIER_1_NODE_IP_TESTNET,
				TIER_1_NODE_PORT_TESTNET,
				secret_key,
				0,
				true,
				true,
				false,
			);
			(node_info, cfg)
		}
	}

	fn get_default_overlay_conn_config(&self) -> (NodeInfo, ConnConfig) {
		// Generate a random keypair for signing messages to the overlay network
		let mut secret_binary = [0u8; 32];
		rand::thread_rng().fill_bytes(&mut secret_binary);
		let secret_key = SecretKey::from_binary(secret_binary);

		self.get_overlay_conn_config(secret_key)
	}

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
				match collector.build_proof(slot, &stellar_sender).await {
					None => {
						sleep(Duration::from_secs(5)).await;
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

	pub fn get_last_slot_index(&self) -> Slot {
		*self.collector.last_slot_index()
	}

	pub fn remove_data(&mut self, slot: &Slot) {
		self.collector.remove_data(slot);
	}

	/// Runs a task to handle messages coming from the Stellar Nodes, and external messages
	async fn run(&mut self, node_info: NodeInfo, conn_config: ConnConfig) -> Result<(), Error> {
		let mut overlay_conn = StellarOverlayConnection::connect(node_info, conn_config).await?;

		let (sender, mut receiver) = mpsc::channel(34);
		self.message_sender = Some(sender.clone());

		let collector = self.collector.clone();
		let actions_sender = overlay_conn.get_actions_sender();
		let r = overlay_conn.disconnect_status();

		// handle a message from the overlay network
		service::spawn_cancelable(self.shutdown_sender.subscribe(), async move {
			let collector = collector.clone();
			loop {
				tokio::select! {
					// runs the stellar-relay and listens to data to collect the scp messages and txsets.
					Some(msg) = overlay_conn.listen() => {
						handle_message(msg, &collector, &sender).await?;
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

		tokio::spawn(on_shutdown(self.shutdown_sender.clone(), async move {
			let result_sending_diconnect = actions_sender.send(r).await.map_err(Error::from);
			if let Err(e) = result_sending_diconnect {
				tracing::error!("Failed to send message to error : {:#?}", e);
			};
		}));

		Ok(())
	}

	/// Starts the agent with the vault's secret key
	pub async fn start_with_secret_key(&mut self, secret_key: SecretKey) -> Result<(), Error> {
		tracing::info!("Starting agent with secret key: {:?}", secret_key);

		let (node_info, conn_config) = self.get_overlay_conn_config(secret_key);

		self.run(node_info, conn_config).await
	}

	/// Starts listening for new SCP messages and stores them in the collector.
	pub async fn start(&mut self) -> Result<(), Error> {
		tracing::info!("Starting agent");
		let (node_info, conn_config) = self.get_default_overlay_conn_config();

		self.run(node_info, conn_config).await
	}

	/// Stops listening for new SCP messages.
	pub fn stop(&mut self) -> Result<(), Error> {
		tracing::info!("Stopping agent");
		if let Err(e) = self.shutdown_sender.send(()) {
			tracing::error!("Failed to send shutdown signal to the agent: {:?}", e);
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use std::fs;

	use crate::oracle::{traits::ArchiveStorage, ScpArchiveStorage, TransactionsArchiveStorage};

	use super::*;

	fn remove_xdr_files(target_slot: u64) {
		let (_, file) = <TransactionsArchiveStorage as ArchiveStorage>::get_url_and_file_name(
			target_slot as u32,
		);
		fs::remove_file(file).expect("should be able to remove the newly added file.");
		let (_, file) =
			<ScpArchiveStorage as ArchiveStorage>::get_url_and_file_name(target_slot as u32);
		fs::remove_file(file).expect("should be able to remove the newly added file.");
	}

	#[tokio::test]
	async fn test_get_proof_for_current_slot() {
		let mut agent = OracleAgent::new(true).unwrap();
		agent.start().await.expect("Failed to start agent");

		// Wait until agent is caught up with the network.
		while agent.get_last_slot_index() == 0 {
			sleep(Duration::from_secs(1)).await;
		}
		let latest_slot = agent.get_last_slot_index();
		println!("Fetching proof for latest slot: {}", latest_slot);

		let proof_result = agent.get_proof(latest_slot).await;
		assert!(proof_result.is_ok(), "Failed to get proof for slot: {}", latest_slot);
	}

	#[tokio::test]
	async fn test_get_proof_for_archived_slot() {
		let mut agent = OracleAgent::new(true).expect("should return an agent");
		agent.start().await.expect("Failed to start agent");

		// This slot should be archived on the public network
		let target_slot = 44041116;
		let proof = agent.get_proof(target_slot).await.expect("should return a proof");

		assert_eq!(proof.slot(), 44041116);

		remove_xdr_files(target_slot);

		agent.stop().expect("Failed to stop the agent");
	}
}
