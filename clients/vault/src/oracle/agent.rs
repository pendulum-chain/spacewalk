use std::time::Duration;

use async_trait::async_trait;
use rand::RngCore;
use tokio::{
	sync::{mpsc::Sender, oneshot},
	time::timeout,
};

use runtime::ShutdownSender;
use stellar_relay_lib::{
	node::NodeInfo,
	sdk::{
		network::{PUBLIC_NETWORK, TEST_NETWORK},
		SecretKey,
	},
	ConnConfig, StellarOverlayConnection,
};

use crate::oracle::{
	collector::ScpMessageCollector, constants::*, errors::Error, types::Slot, ActorMessage, Proof,
	ScpMessageActor,
};

#[async_trait]
pub trait OracleAgent {
	/// This method returns the proof for a given slot or an error if the proof cannot be provided.
	/// The agent will try every possible way to get the proof before returning an error.
	async fn get_proof(&self, slot: Slot) -> Result<Proof, Error>;

	/// Starts listening for new SCP messages and stores them in the collector.
	async fn start(&mut self) -> Result<(), Error>;

	/// Stops listening for new SCP messages.
	async fn stop(&mut self) -> Result<(), Error>;
}

pub struct Agent {
	actor_action_sender: Sender<ActorMessage>,
	actor: ScpMessageActor,
	overlay_conn: Option<StellarOverlayConnection>,
	pub is_public_network: bool,
	shutdown_sender: ShutdownSender,
}

impl Agent {
	fn new(is_public_network: bool) -> Self {
		let (sender, receiver) = tokio::sync::mpsc::channel(1024);
		let collector = ScpMessageCollector::new(is_public_network);

		let actor = ScpMessageActor::new(receiver, collector);
		let shutdown_sender = ShutdownSender::default();

		Self {
			actor_action_sender: sender,
			actor,
			is_public_network,
			shutdown_sender,
			overlay_conn: None,
		}
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
}

#[async_trait]
impl OracleAgent for Agent {
	async fn get_proof(&self, slot: Slot) -> Result<Proof, Error> {
		tracing::info!("Getting proof for slot {}", slot);
		let (sender, receiver) = oneshot::channel();
		self.actor_action_sender.send(ActorMessage::GetProof { slot, sender }).await?;

		let future = async {
			let proof = receiver.await?;
			Ok(proof)
		};

		timeout(Duration::from_secs(10), future.await)
			.await
			.map_err(|_| Error::ProofTimeout("Getting the proof timed out.".to_string()))
	}

	async fn start(&mut self) -> Result<(), Error> {
		tracing::info!("Starting agent");
		let (node_info, conn_config) = self.get_default_overlay_conn_config();
		let overlay_conn = StellarOverlayConnection::connect(node_info, conn_config).await?;
		self.overlay_conn = Some(overlay_conn.clone());

		service::spawn_cancelable(self.shutdown_sender.subscribe(), async move {
			self.actor.run(overlay_conn).await?;
		});
		Ok(())
	}

	async fn stop(&mut self) -> Result<(), Error> {
		tracing::info!("Stopping agent");
		// self.overlay_conn.disconnect();
		if let Err(e) = self.shutdown_sender.send(()).await {
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

		let mut agent = Agent::new(true);
		agent.start().await.expect("Failed to start agent");

		// TODO get the current slot from the network
		let target_slot = 573112;
		let proof = agent.get_proof(target_slot).await.unwrap();

		assert_eq!(proof.slot(), 1);
		agent.stop().await.expect("Failed to stop the agent");
	}

	#[tokio::test]
	async fn test_get_proof_for_archived_slot() {
		prepare_directories().expect("Failed to prepare directories.");

		let mut agent = Agent::new(true);
		agent.start().await.expect("Failed to start agent");

		// This slot should be archived on the public network
		let target_slot = 573112;
		let proof = agent.get_proof(target_slot).await.unwrap();

		assert_eq!(proof.slot(), 1);
		agent.stop().await.expect("Failed to stop the agent");
	}
}
