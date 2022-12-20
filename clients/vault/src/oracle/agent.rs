use std::{
	sync::Arc,
	time::Duration,
};


use rand::RngCore;
use tokio::{
	sync::{mpsc},
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
	collector::ScpMessageCollector, constants::*, errors::Error, prepare_directories, types::Slot,
	Proof,
};

pub struct OracleAgent {
	collector: Arc<ScpMessageCollector>,
	pub is_public_network: bool,
	message_sender: Option<mpsc::Sender<StellarMessage>>,
	shutdown_sender: ShutdownSender,
}

/// listens to data to collect the scp messages and txsets.
async fn handle_message(
	message: StellarRelayMessage,
	collector: &Arc<ScpMessageCollector>,
	message_sender: &mpsc::Sender<StellarMessage>,
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
		// todo
		StellarRelayMessage::Connect { pub_key: _, node_info: _ } => {},
		// todo
		StellarRelayMessage::Error(_) => {},
		// todo
		StellarRelayMessage::Timeout => {},
	}

	Ok(())
}

impl OracleAgent {
	pub fn new(is_public_network: bool) -> Result<Self, Error> {
		let collector = Arc::new(ScpMessageCollector::new(is_public_network));
		let shutdown_sender = ShutdownSender::default();
		prepare_directories()?;
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
			.ok_or(Error::Uninitialized("MessageSender".to_string()))?;

		let collector = self.collector.clone();

		timeout(Duration::from_secs(80), async move {
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

	async fn run(&mut self, node_info: NodeInfo, conn_config: ConnConfig) -> Result<(), Error> {
		let mut overlay_conn = StellarOverlayConnection::connect(node_info, conn_config).await?;

		let (sender, mut receiver) = mpsc::channel(34);
		self.message_sender = Some(sender.clone());

		let collector = self.collector.clone();

		// handle a message from the overlay network
		service::spawn_cancelable(self.shutdown_sender.subscribe(), async move {
			let collector = collector.clone();
			loop {
				tokio::select! {
					// runs the stellar-relay and listens to data to collect the scp messages and txsets.

					Some(msg) = overlay_conn.listen() => {
						handle_message(msg,&collector,&sender).await?;
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
		// self.overlay_conn.disconnect();
		if let Err(e) = self.shutdown_sender.send(()) {
			tracing::error!("Failed to send shutdown signal to the agent: {:?}", e);
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	

	

	// #[tokio::test]
	// async fn test_get_proof_for_current_slot() {
	// 	prepare_directories().expect("Failed to prepare directories.");
	//
	// 	let mut agent = OracleAgent::new(true).unwrap();
	// 	agent.start().await.expect("Failed to start agent");
	//
	// 	tokio::time::sleep(Duration::from_secs(3)).await;
	// 	let slot = 44041116;
	//
	// 	match agent.get_proof(slot).await {
	// 		Ok(proof) => {
	// 			println!("proof: {:?}", proof);
	// 			assert!(true)
	// 		},
	// 		Err(e) => assert!(false),
	// 	}
	// }

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
