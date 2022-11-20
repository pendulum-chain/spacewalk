use tokio::sync::{mpsc, oneshot};

use stellar_relay::{
	node::NodeInfo, sdk::types::StellarMessage, ConnConfig, StellarOverlayConnection,
	StellarRelayMessage,
};

use crate::oracle::{
	collector::{Proof, ProofStatus, ScpMessageCollector},
	errors::Error,
	storage::prepare_directories,
	types::Slot,
};

/// A message used to communicate with the Actor
pub enum ActorMessage {
	/// returns the envelopes map size.
	CurrentMapSize {
		sender: oneshot::Sender<usize>,
	},

	/// Watch out for scpenvelopes and txsets for the given transaction (from Horizon)
	WatchSlot {
		slot: Slot,
	},

	GetProof {
		slot: Slot,
		sender: oneshot::Sender<ProofStatus>,
	},
	/// Gets all proofs
	GetPendingProofs {
		sender: oneshot::Sender<Vec<Proof>>,
	},

	GetLastSlotIndex {
		sender: oneshot::Sender<Slot>,
	},
}

/// Runs both the stellar-relay and its own.
struct ScpMessageActor {
	/// used to receive messages from outside the actor.
	action_receiver: mpsc::Receiver<ActorMessage>,
	collector: ScpMessageCollector,
}

impl ScpMessageActor {
	fn new(receiver: mpsc::Receiver<ActorMessage>, collector: ScpMessageCollector) -> Self {
		ScpMessageActor { action_receiver: receiver, collector }
	}

	/// handles messages sent from the outside.
	async fn handle_message(&mut self, msg: ActorMessage, overlay_conn: &StellarOverlayConnection) {
		match msg {
			ActorMessage::CurrentMapSize { sender } => {
				let _ = sender.send(self.collector.envelopes_map_len());
			},

			ActorMessage::GetPendingProofs { sender } => {
				let _ = sender.send(self.collector.get_pending_proofs(overlay_conn).await);
			},

			ActorMessage::WatchSlot { slot } => {
				// watch out for this slot
				self.collector.watch_slot(slot);
			},

			ActorMessage::GetProof { slot, sender } => {
				let _ = sender.send(self.collector.build_proof(slot, overlay_conn).await);
			},

			ActorMessage::GetLastSlotIndex { sender } => {
				let res = self.collector.last_slot_index();
				if let Err(slot) = sender.send(*res) {
					tracing::warn!("failed to send back the slot number: {}", slot);
				}
			},
		};
	}

	/// runs the stellar-relay and listens to data to collect the scp messages and txsets.
	async fn run(&mut self, mut overlay_conn: StellarOverlayConnection) -> Result<(), Error> {
		loop {
			tokio::select! {
				// listen to stellar node
				Some(conn_state) = overlay_conn.listen() => {
					match conn_state {
						StellarRelayMessage::Data {
							p_id: _,
							msg_type: _,
							msg,
						} => match msg {
							StellarMessage::ScpMessage(env) => {
								self.collector
									.handle_envelope(env, &overlay_conn)
									.await?;
							}
							StellarMessage::TxSet(set) => {
								self.collector.handle_tx_set(&set);
							}
							_ => {}
						},
						// todo
						StellarRelayMessage::Connect{ pub_key: _, node_info: _ }  => {},
						// todo
						StellarRelayMessage::Error(_) => {}
						// todo
						StellarRelayMessage::Timeout => {}
					}
				}
				// handle message from user
				Some(msg) = self.action_receiver.recv() => {
						self.handle_message(msg, &overlay_conn).await;
				}
			}
		}
	}
}

/// Handler to communicate with the ScpMessageActor
#[derive(Clone)]
pub struct ScpMessageHandler {
	action_sender: mpsc::Sender<ActorMessage>,
	pub is_public_network: bool,
}

impl ScpMessageHandler {
	/// creates a new Handler.
	/// owns the Actor that runs the StellarOverlayConnection
	fn new(
		overlay_conn: StellarOverlayConnection,
		vault_addresses: Vec<String>,
		is_public_network: bool,
	) -> Self {
		let (sender, receiver) = mpsc::channel(1024);
		let collector = ScpMessageCollector::new(is_public_network, vault_addresses);

		let mut actor = ScpMessageActor::new(receiver, collector);
		tokio::spawn(async move { actor.run(overlay_conn).await });

		Self { action_sender: sender, is_public_network }
	}

	/// A sample method to communicate with the actor.
	/// Returns the size of the EnvelopesMap of the ScpMessageCollector.
	pub async fn get_size(&self) -> Result<usize, Error> {
		let (sender, receiver) = oneshot::channel();

		self.action_sender.send(ActorMessage::CurrentMapSize { sender }).await?;

		receiver.await.map_err(Error::from)
	}

	/// Returns a list of transactions with each of their corresponding proofs
	pub async fn get_pending_proofs(&self) -> Result<Vec<Proof>, Error> {
		let (sender, receiver) = oneshot::channel();

		self.action_sender.send(ActorMessage::GetPendingProofs { sender }).await?;

		receiver.await.map_err(Error::from)
	}

	pub async fn watch_slot(&self, slot: Slot) -> Result<(), Error> {
		self.action_sender.send(ActorMessage::WatchSlot { slot }).await?;
		Ok(())
	}

	pub fn handle_issue_event(&self) {
		todo!();
	}

	pub fn handle_redeem_event(&self) {
		todo!();
	}

	pub async fn get_proof(&self, slot: Slot) -> Result<ProofStatus, Error> {
		let (sender, receiver) = oneshot::channel();
		self.action_sender.send(ActorMessage::GetProof { slot, sender }).await?;

		receiver.await.map_err(Error::from)
	}

	pub async fn get_last_slot_index(&self) -> Result<Slot, Error> {
		let (sender, receiver) = oneshot::channel();
		self.action_sender.send(ActorMessage::GetLastSlotIndex { sender }).await?;
		receiver.await.map_err(Error::from)
	}
}

/// Creates the ScpMessageHandler and contains the thread that connects and listens to the Stellar
/// Node
///
/// # Arguments
///
/// * `node_info` - Information (w/o the address and port) of the Stellar Node to connect to.
/// * `connection_cfg` - The configuration on how and what (address and port) Stellar Node to
///   connect to.
/// * `is_public_network` - Determines whether the network we'll connect to is public or not
/// * `vault_addresses` - the addresses of this vault
pub async fn create_handler(
	node_info: NodeInfo,
	connection_cfg: ConnConfig,
	is_public_network: bool,
	vault_addresses: Vec<String>,
) -> Result<ScpMessageHandler, Error> {
	prepare_directories()?;

	let overlay_connection = StellarOverlayConnection::connect(node_info, connection_cfg).await?;

	Ok(ScpMessageHandler::new(overlay_connection, vault_addresses, is_public_network))
}
