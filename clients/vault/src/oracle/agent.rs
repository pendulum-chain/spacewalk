use std::{
	fmt::{Debug, Display, Formatter},
	sync::Arc,
	thread::sleep,
	time::Duration,
};

use async_trait::async_trait;
use rand::RngCore;
use tokio::{
	sync::{mpsc, oneshot, RwLock, RwLockReadGuard},
	time::timeout,
};

use crate::ArcRwLock;
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
	collector::ScpMessageCollector, constants::*, errors::Error, prepare_directories, types::Slot,
	Proof, ProofStatus,
};

pub struct OracleAgent {
	collector: ArcRwLock<ScpMessageCollector>,
	pub is_public_network: bool,
	message_sender: Option<mpsc::Sender<StellarMessage>>,
	shutdown_sender: ShutdownSender,
}

/// listens to data to collect the scp messages and txsets.
async fn handle_message(
	message: StellarRelayMessage,
	collector: ArcRwLock<ScpMessageCollector>,
	message_sender: &mpsc::Sender<StellarMessage>,
) -> Result<(), Error> {
	match message {
		StellarRelayMessage::Data { p_id, msg_type, msg } => match msg {
			StellarMessage::ScpMessage(env) => {
				let mut collector_w = collector.write().await;
				collector_w.handle_envelope(env, message_sender).await?;
				drop(collector_w);
			},
			StellarMessage::TxSet(set) => {
				let mut collector_w = collector.write().await;
				collector_w.handle_tx_set(set);
				drop(collector_w);
			},
			_ => {},
		},
		// todo
		StellarRelayMessage::Connect { pub_key, node_info } => {},
		// todo
		StellarRelayMessage::Error(_) => {},
		// todo
		StellarRelayMessage::Timeout => {},
	}

	Ok(())
}

impl OracleAgent {
	pub fn new(is_public_network: bool) -> Result<Self, Error> {
		let collector = Arc::new(RwLock::new(ScpMessageCollector::new(is_public_network)));
		let shutdown_sender = ShutdownSender::default();
		prepare_directories()?;
		Ok(Self { collector, is_public_network, message_sender: None, shutdown_sender })
	}

	fn get_default_overlay_conn_config(&self) -> (NodeInfo, ConnConfig) {
		// Generate a random keypair for signing messages to the overlay network
		let mut secret_binary = [0u8; 32];
		rand::thread_rng().fill_bytes(&mut secret_binary);
		let secret_key = SecretKey::from_binary(secret_binary);

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

	/// This method returns the proof for a given slot or an error if the proof cannot be provided.
	/// The agent will try every possible way to get the proof before returning an error.
	/// Set timeout to 60 seconds; 10 seconds interval.
	pub async fn get_proof(&mut self, slot: Slot) -> Result<Proof, Error> {
		let mut num_of_retries = 0;

		let sender = self
			.message_sender
			.as_ref()
			.ok_or(Error::Uninitialized("MessageSender".to_string()))?;

		while num_of_retries < 6 {
			println!("retry attempt #{}", num_of_retries);
			match self.collector.write().await.build_proof(slot, sender).await {
				None => {
					num_of_retries += 1;
					sleep(Duration::from_secs(10));
				},
				Some(proof) => return Ok(proof),
			}
		}

		Err(Error::ProofTimeout("Proof not found after 6 attempts.".to_string()))
	}

	pub async fn get_last_slot_index(&self) -> Slot {
		let collector = self.collector.read().await;
		let last_slot_idx = collector.last_slot_index();

		*last_slot_idx
	}

	pub async fn remove_data(&mut self, slot: &Slot) {
		self.collector.write().await.remove_data(slot);
	}

	/// Starts listening for new SCP messages and stores them in the collector.
	pub async fn start(&mut self) -> Result<(), Error> {
		tracing::info!("Starting agent");
		let (node_info, conn_config) = self.get_default_overlay_conn_config();
		let mut overlay_conn = StellarOverlayConnection::connect(node_info, conn_config).await?;

		let (sender, mut receiver) = mpsc::channel(34);
		self.message_sender = Some(sender.clone());

		let collector = self.collector.clone();

		// handle a message from the overlay network
		service::spawn_cancelable(self.shutdown_sender.subscribe(), async move {
			loop {
				tokio::select! {
					// runs the stellar-relay and listens to data to collect the scp messages and txsets.

					Some(msg) = overlay_conn.listen() => {
						handle_message(msg,collector.clone(),&sender).await?;
					},

					Some(msg) = receiver.recv() => {
						sender.clone().send(msg).await?;

					}
				}
			}
			Ok::<(), Error>(())
		});

		Ok(())
	}

	/// Stops listening for new SCP messages.
	pub async fn stop(&mut self) -> Result<(), Error> {
		tracing::info!("Stopping agent");
		// self.overlay_conn.disconnect();
		if let Err(e) = self.shutdown_sender.send(()) {
			tracing::error!("Failed to send shutdown signal to the agent: {:?}", e);
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use crate::oracle::storage::prepare_directories;

	use super::*;

	#[tokio::test]
	async fn test_get_proof_for_current_slot() {
		prepare_directories().expect("Failed to prepare directories.");

		let mut agent = OracleAgent::new(true).unwrap();
		agent.start().await.expect("Failed to start agent");

		tokio::time::sleep(Duration::from_secs(3)).await;
		let slot = 44041116;

		match agent.get_proof(slot).await {
			Ok(proof) => {
				println!("proof: {:?}", proof);
				assert!(true)
			},
			Err(e) => assert!(false),
		}
	}

	// #[tokio::test]
	// async fn test_get_proof_for_archived_slot() {
	// 	prepare_directories().expect("Failed to prepare directories.");
	//
	// 	let mut agent = OracleAgent::new(true).expect("should return an agent");
	// 	agent.start().await.expect("Failed to start agent");
	//
	// 	// This slot should be archived on the public network
	// 	let target_slot = 573112;
	// 	let proof = agent.get_proof(target_slot).await.unwrap();
	//
	// 	assert_eq!(proof.slot(), 1);
	// 	agent.stop().await.expect("Failed to stop the agent");
	// }
}
