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
		types::StellarMessage,
		SecretKey,
	},
	ConnConfig, StellarOverlayConnection, StellarRelayMessage,
};

use crate::oracle::{
	collector::ScpMessageCollector, constants::*, errors::Error, prepare_directories, types::Slot,
	Proof, ProofStatus,
};

/// A message used to communicate with the Actor
pub enum ActorMessage {
	GetProof { slot: Slot, sender: oneshot::Sender<Proof> },
	GetLastSlotIndex { sender: oneshot::Sender<Slot> },
	RemoveData { slot: Slot },
}


pub struct OracleAgent {
	actor_action_sender: Sender<ActorMessage>,
	collector: ScpMessageCollector,
	overlay_conn: Option<StellarOverlayConnection>,
	pub is_public_network: bool,
	shutdown_sender: ShutdownSender,
}


#[async_trait]
trait MessageHandler<T> {
	async fn handle_message(&mut self, message:T) -> Result<(),Error>;
}

#[async_trait]
impl MessageHandler<ActorMessage> for OracleAgent {
	async fn handle_message(&mut self, message: ActorMessage) -> Result<(),Error> {
		if self.overlay_conn.is_none() {
			return Ok(());
		}

		let overlay_conn = self.overlay_conn.as_ref().expect("overlay_conn cannot be None");

		match message {
			ActorMessage::GetProof { slot, sender } => {
				let proof = self.collector.build_proof(slot, overlay_conn).await;
				match proof {
					Ok(proof) => {
						let _ = sender.send(proof);
					},
					Err(e) => {
						tracing::error!("Error while building proof: {}", e);
						// TODO maybe change this to send an error back to the sender
					},
				}
			},
			ActorMessage::GetLastSlotIndex { sender } => {
				let res = self.collector.last_slot_index();
				if let Err(slot) = sender.send(*res) {
					tracing::warn!("failed to send back the slot number: {}", slot);
				}
			},
			ActorMessage::RemoveData { slot } => {
				self.collector.remove_data(&slot);
			},
		};

		Ok(())
	}
}

#[async_trait]
impl MessageHandler<StellarRelayMessage> for OracleAgent {
	async fn handle_message(&mut self, message: StellarRelayMessage) -> Result<(),Error> {
		if self.overlay_conn.is_none() {
			return Ok(());
		}
		let overlay_conn = self.overlay_conn.as_ref().expect("overlay_conn cannot be None");

		match message {
			StellarRelayMessage::Data { p_id: _, msg_type: _, msg } => match msg {
				StellarMessage::ScpMessage(env) => {
					self.collector.handle_envelope(env, &overlay_conn).await?;
				},
				StellarMessage::TxSet(set) => {
					self.collector.handle_tx_set(set);
				},
				_ => {},
			},
			// todo
			StellarRelayMessage::Connect { pub_key: _, node_info: _ } => {},
			// todo
			StellarRelayMessage::Error(_) => {},
			// todo
			StellarRelayMessage::Timeout => {},
		}

		Ok(())
	}
}


impl OracleAgent {
	pub(crate) fn new(is_public_network: bool) -> Result<Self,Error> {
		let (sender, receiver) = tokio::sync::mpsc::channel(1024);
		let collector = ScpMessageCollector::new(is_public_network);

		let shutdown_sender = ShutdownSender::default();

		prepare_directories()?;

		Ok(
			Self {
				actor_action_sender: sender,
				collector,
				is_public_network,
				shutdown_sender,
				overlay_conn: None,
			}
		)
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

	/// runs the stellar-relay and listens to data to collect the scp messages and txsets.
	async fn run(&mut self) -> Result<(), Error> {
		if self.overlay_conn.is_none() {
			return Ok(())
		}
		let overlay_conn = self.overlay_conn.as_ref().expect("overlay_conn cannot be None");

		// Periodically either handle a message from the overlay network or from the 'user' that
		// sends an actor-message we should act upon
		service::spawn_cancelable(self.shutdown_sender.subscribe(), async move {
			loop {
				tokio::select! {
					Some(msg) = overlay_conn.listen() => {
							self.handle_message(msg).await?;
					},
					Some(msg) = self.action_receiver.recv() => {
							self.handle_message(msg).await?;
					}
				}
			}
		})
		.await;

		Ok(())
	}

	/// This method returns the proof for a given slot or an error if the proof cannot be provided.
	/// The agent will try every possible way to get the proof before returning an error.

	pub async fn get_proof(&self, slot: Slot) -> Result<Proof, Error> {
		tracing::info!("Getting proof for slot {}", slot);
		let (sender, receiver) = oneshot::channel();
		self.actor_action_sender.send(ActorMessage::GetProof { slot, sender }).await?;

		let future = async {
			let proof = receiver.await?;
			Ok(proof)
		};

		timeout(Duration::from_secs(10), future)
			.await
			.map_err(|_| Error::ProofTimeout("Getting the proof timed out.".to_string()))
			.and_then(|res| res )
	}

	/// Starts listening for new SCP messages and stores them in the collector.
	pub async fn start(&mut self) -> Result<(), Error> {
		tracing::info!("Starting agent");
		let (node_info, conn_config) = self.get_default_overlay_conn_config();
		let overlay_conn = StellarOverlayConnection::connect(node_info, conn_config).await?;
		self.overlay_conn = Some(overlay_conn);

		self.run();
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
