use std::convert::{TryFrom, TryInto};

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use stellar_relay_lib::{
	node::NodeInfo, sdk::types::StellarMessage, ConnConfig, StellarOverlayConnection,
	StellarRelayMessage,
};
use wallet::types::Watcher;

use crate::oracle::{
	collector::{Proof, ProofExt, ProofStatus, ScpMessageCollector},
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

	GetSlotWatchList {
		sender: oneshot::Sender<Vec<Slot>>,
	},

	RemoveData {
		slot: Slot,
	},
	GetScpState {
		missed_slot: u64,
	},
	Disconnect,
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
			ActorMessage::GetSlotWatchList { sender } => {
				let _ = sender.send(self.collector.get_slot_watch_list());
			},
			ActorMessage::RemoveData { slot } => {
				self.collector.remove_data(&slot);
			},
			ActorMessage::GetScpState { missed_slot } => {
				overlay_conn
					.send(StellarMessage::GetScpState(missed_slot.try_into().unwrap()))
					.await;
			},
			ActorMessage::Disconnect => {
				panic!("Should disconnect from run method")
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
								self.collector.handle_tx_set(set);
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
					match msg{
						ActorMessage::Disconnect => {
							overlay_conn.disconnect().await;
						},
						_ => {
							self.handle_message(msg, &overlay_conn).await;
						}
					}
				}
				else => continue,
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
	fn new(overlay_conn: StellarOverlayConnection, is_public_network: bool) -> Self {
		let (sender, receiver) = mpsc::channel(1024);
		let collector = ScpMessageCollector::new(is_public_network, sender.clone());

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

	pub async fn get_last_slot_index(&self) -> Result<Slot, Error> {
		let (sender, receiver) = oneshot::channel();
		self.action_sender.send(ActorMessage::GetLastSlotIndex { sender }).await?;
		receiver.await.map_err(Error::from)
	}

	pub async fn get_slot_watchlist(&self) -> Result<Vec<Slot>, Error> {
		let (sender, receiver) = oneshot::channel();
		self.action_sender.send(ActorMessage::GetSlotWatchList { sender }).await?;
		receiver.await.map_err(Error::from)
	}

	/// creates a struct that will send a `watch_slot` message to oracle.
	pub fn create_watcher(&self) -> OracleWatcher {
		OracleWatcher { action_sender: self.action_sender.clone() }
	}

	/// creates a struct that will send proof related messages to oracle.
	pub fn proof_operations(&self) -> OracleProofOps {
		OracleProofOps { action_sender: self.action_sender.clone() }
	}

	pub async fn disconnect(&self) -> Result<(), Error> {
		self.action_sender.send(ActorMessage::Disconnect).await.map_err(Error::from)
	}
}

#[derive(Clone)]
pub struct OracleWatcher {
	action_sender: mpsc::Sender<ActorMessage>,
}

#[async_trait]
impl Watcher for OracleWatcher {
	async fn watch_slot(&self, slot: u128) -> Result<(), wallet::error::Error> {
		let u64_slot = Slot::try_from(slot).map_err(|_| {
			tracing::error!("Failed to convert slot {} of type u128 to u64", slot);
			wallet::error::Error::OracleError
		})?;

		self.action_sender
			.send(ActorMessage::WatchSlot { slot: u64_slot })
			.await
			.map_err(|e| {
				tracing::error!("Failed to send {:?} message to Oracle.", e.to_string());
				wallet::error::Error::OracleError
			})
	}
}

pub struct OracleProofOps {
	action_sender: mpsc::Sender<ActorMessage>,
}

#[async_trait]
impl ProofExt for OracleProofOps {
	async fn get_proof(&self, slot: Slot) -> Result<ProofStatus, Error> {
		let (sender, receiver) = oneshot::channel();
		self.action_sender.send(ActorMessage::GetProof { slot, sender }).await?;

		receiver.await.map_err(Error::from)
	}

	async fn get_pending_proofs(&self) -> Result<Vec<Proof>, Error> {
		let (sender, receiver) = oneshot::channel();
		self.action_sender.send(ActorMessage::GetPendingProofs { sender }).await?;

		receiver.await.map_err(Error::from)
	}

	async fn processed_proof(&self, slot: Slot) {
		if let Err(e) = self.action_sender.send(ActorMessage::RemoveData { slot }).await {
			tracing::warn!(
				"Failed to send RemoveData action for slot {}: {:?}",
				slot,
				e.to_string()
			);
		}
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
pub async fn create_handler(
	node_info: NodeInfo,
	connection_cfg: ConnConfig,
	is_public_network: bool,
) -> Result<ScpMessageHandler, Error> {
	prepare_directories()?;

	let overlay_connection = StellarOverlayConnection::connect(node_info, connection_cfg).await?;

	Ok(ScpMessageHandler::new(overlay_connection, is_public_network))
}
