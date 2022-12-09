use async_trait::async_trait;
use rand::RngCore;
use tokio::sync::{mpsc::Sender, oneshot};

use stellar_relay_lib::{node::NodeInfo, sdk::SecretKey, ConnConfig, StellarOverlayConnection};

use crate::{
	oracle::{collector::ScpMessageCollector, types::Slot, ActorMessage, Proof, ScpMessageActor},
	Error,
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
	action_sender: Sender<ActorMessage>,
	overlay_conn: Option<StellarOverlayConnection>,
	pub is_public_network: bool,
}

impl Agent {
	fn new(is_public_network: bool) -> Self {
		let (sender, receiver) = tokio::sync::mpsc::channel(1024);
		let collector = ScpMessageCollector::new(is_public_network);

		let mut actor = ScpMessageActor::new(receiver, collector);

		tokio::spawn(async move { actor.run().await });

		Self { action_sender: sender, is_public_network, overlay_conn: None }
	}

	fn get_default_overlay_conn_config(&self) -> (NodeInfo, ConnConfig) {
		// Generate a random keypair for signing messages to the overlay network
		let mut secret_binary = [0u8; 32];
		rand::thread_rng().fill_bytes(&mut secret_binary);
		let secret_key = SecretKey::from_binary(secret_binary);

		if self.is_public_network {
			let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), network);
			let cfg = ConnConfig::new(tier1_node_ip, 11625, secret_key, 0, true, true, false);
			(node_info, cfg)
		} else {
			let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), network);
			let cfg = ConnConfig::new(tier1_node_ip, 11625, secret_key, 0, true, true, false);
			(node_info, cfg)
		}
	}
}

#[async_trait]
impl OracleAgent for Agent {
	async fn get_proof(&self, slot: Slot) -> Result<Proof, Error> {
		let (sender, receiver) = oneshot::channel();
		self.action_sender.send(ActorMessage::GetProof { slot, sender }).await?;

		receiver.await.map_err(Error::from)
	}

	async fn start(&mut self) -> Result<(), Error> {
		let (node_info, conn_config) = self.get_default_overlay_conn_config();
		let overlay_conn = StellarOverlayConnection::connect(node_info, conn_config).await?;
		self.overlay_conn = Some(overlay_conn);

		Ok(())
	}

	async fn stop(&mut self) -> Result<(), Error> {
		// self.overlay_conn.disconnect();
		Ok(())
	}
}
