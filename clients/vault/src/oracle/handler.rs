use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};

use stellar_relay::{
	node::NodeInfo,
	sdk::{types::StellarMessage, TransactionEnvelope},
	ConnConfig, StellarOverlayConnection, StellarRelayMessage,
};

use crate::oracle::{
	collector::{Proof, ScpMessageCollector},
	errors::Error,
	storage::prepare_directories,
	types::{TxEnvelopeFilter, TxSetToSlotMap},
	FilterWith, TxFilterMap,
};

/// A message used to communicate with the Actor
pub enum ActorMessage {
	/// returns the envelopes map size.
	CurrentMapSize {
		sender: oneshot::Sender<usize>,
	},
	/// filters on the transaction we want to process.
	AddFilter {
		filter: Box<dyn FilterWith<TransactionEnvelope> + Send + Sync>,
	},

	RemoveFilter(String),
	/// Gets all proofs
	GetPendingProofs {
		sender: oneshot::Sender<Vec<Proof>>,
	},
}

/// Runs both the stellar-relay and its own.
struct ScpMessageActor {
	/// used to receive messages from outside the actor.
	action_receiver: mpsc::Receiver<ActorMessage>,
	collector: ScpMessageCollector,
	/// the filters used to filter out transactions for processing.
	tx_env_filters: TxFilterMap,
}

impl ScpMessageActor {
	fn new(receiver: mpsc::Receiver<ActorMessage>, collector: ScpMessageCollector) -> Self {
		ScpMessageActor { action_receiver: receiver, collector, tx_env_filters: HashMap::new() }
	}

	/// handles messages sent from the outside.
	async fn handle_message(&mut self, msg: ActorMessage) {
		match msg {
			ActorMessage::CurrentMapSize { sender } => {
				let _ = sender.send(self.collector.envelopes_map_len());
			},

			ActorMessage::AddFilter { filter } => {
				tracing::info!("adding filter: {}", filter.name());
				self.tx_env_filters.insert(filter.name(), filter);
			},

			ActorMessage::RemoveFilter(name) => {
				let _ = self.tx_env_filters.remove(&name);
			},

			ActorMessage::GetPendingProofs { sender } => {
				let _ = sender.send(self.collector.get_pending_proofs());
			},
		};
	}

	/// runs the stellar-relay and listens to data to collect the scp messages and txsets.
	async fn run(&mut self, mut overlay_conn: StellarOverlayConnection) -> Result<(), Error> {
		let mut tx_set_to_slot_map: TxSetToSlotMap = HashMap::new();

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
									.handle_envelope(env, &mut tx_set_to_slot_map, &overlay_conn)
									.await?;
							}
							StellarMessage::TxSet(set) => {
								self.collector.handle_tx_set(&set, &mut tx_set_to_slot_map, &self.tx_env_filters)?;
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
						self.handle_message(msg).await;
				}
			}
		}
	}
}

/// Handler to communicate with the ScpMessageActor
pub struct ScpMessageHandler {
	action_sender: mpsc::Sender<ActorMessage>,
}

impl ScpMessageHandler {
	/// creates a new Handler.
	/// owns the Actor that runs the StellarOverlayConnection
	fn new(
		overlay_conn: StellarOverlayConnection,
		vault_addresses: Vec<String>,
		is_public_network: bool,
	) -> Self {
		let collector = ScpMessageCollector::new(is_public_network, vault_addresses);

		let (sender, receiver) = mpsc::channel(1024);

		let mut actor = ScpMessageActor::new(receiver, collector);
		tokio::spawn(async move { actor.run(overlay_conn).await });

		Self { action_sender: sender }
	}

	/// A sample method to communicate with the actor.
	/// Returns the size of the EnvelopesMap of the ScpMessageCollector.
	pub async fn get_size(&self) -> Result<usize, Error> {
		let (sender, receiver) = oneshot::channel();

		self.action_sender.send(ActorMessage::CurrentMapSize { sender }).await?;

		receiver.await.map_err(Error::from)
	}

	/// Adds a filter on what transactions to process.
	/// Returns an index of the filter in the map.
	pub async fn add_filter(&self, filter: Box<TxEnvelopeFilter>) -> Result<(), Error> {
		self.action_sender
			.send(ActorMessage::AddFilter { filter })
			.await
			.map_err(Error::from)
	}

	/// Removes an existing filter based on its id/key in the map.
	pub async fn remove_filter(&self, filter_name: String) -> Result<(), Error> {
		self.action_sender
			.send(ActorMessage::RemoveFilter(filter_name))
			.await
			.map_err(Error::from)
	}

	/// Returns a list of transactions with each of their corresponding proofs
	pub async fn get_pending_proofs(&self) -> Result<Vec<Proof>, Error> {
		let (sender, receiver) = oneshot::channel();

		self.action_sender.send(ActorMessage::GetPendingProofs { sender }).await?;

		receiver.await.map_err(Error::from)
	}

	pub fn handle_issue_event(&self) {
		todo!();
	}

	pub fn handle_redeem_event(&self) {
		todo!();
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
