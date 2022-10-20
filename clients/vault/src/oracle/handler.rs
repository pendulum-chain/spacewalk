
use std::collections::HashMap;
use stellar_relay::{
	node::NodeInfo, sdk::types::StellarMessage, ConnConfig, StellarOverlayConnection,
	StellarRelayMessage,
};
use tokio::sync::{mpsc, oneshot};
use stellar_relay::sdk::TransactionEnvelope;
use crate::oracle::collector::{EncodedProof, ScpMessageCollector};
use crate::oracle::errors::Error;
use crate::oracle::storage::prepare_directories;
use crate::oracle::{FilterWith, TxFilterMap};
use crate::oracle::types::{TxEnvelopeFilter, TxSetCheckerMap};

pub struct NoFilter;

// Dummy filter that does nothing.
impl FilterWith<TransactionEnvelope> for NoFilter {
	fn id(&self) -> u32 {
		1
	}

	fn check_for_processing(&self, _param: &TransactionEnvelope) -> bool {
		false
	}
}

/// A message used to communicate with the Actor
pub enum ActorMessage {
	/// returns the envelopes map size.
	CurrentMapSize {
		sender: oneshot::Sender<usize>,
	},
	/// filters on the transaction we want to process.

	AddFilter{
		filter:Box<dyn FilterWith<TransactionEnvelope> + Send + Sync>
	},

	RemoveFilter(u32),
	/// Gets all Pending Transactions with Proofs
	GetPendingTx {
		sender: oneshot::Sender<Vec<EncodedProof>>,
	},
}

/// Runs both the stellar-relay and its own.
struct ScpMessageActor {
	/// used to receive messages from outside the actor.
	receiver: mpsc::Receiver<ActorMessage>,
	collector: ScpMessageCollector,
	/// the filters used to filter out transactions for processing.
	tx_env_filters: TxFilterMap
}

impl ScpMessageActor {
	fn new(receiver: mpsc::Receiver<ActorMessage>, collector: ScpMessageCollector) -> Self {
		ScpMessageActor { receiver, collector, tx_env_filters: HashMap::new() }
	}

	/// handles messages sent from the outside.
	async fn handle_message(&mut self, msg: ActorMessage) {
		match msg {
			ActorMessage::CurrentMapSize { sender } => {
				let _ = sender.send(self.collector.envelopes_map_len());
			},

			ActorMessage::AddFilter{ filter} => {
				self.tx_env_filters.insert(filter.id(),filter);

			},

			ActorMessage::RemoveFilter(idx) => {
				let _ = self.tx_env_filters.remove(&idx);
			}

			ActorMessage::GetPendingTx { sender } => {
				let _ = sender.send(self.collector.get_pending_txs());
			},
		};
	}

	/// runs the stellar-relay and listens to data to collect the scp messages and txsets.
	async fn run(&mut self, mut overlay_conn: StellarOverlayConnection) -> Result<(), Error> {
		let mut tx_set_hash_map: TxSetCheckerMap = HashMap::new();

		loop {
			tokio::select! {
				Some(conn_state) = overlay_conn.listen() => {
					match conn_state {
						StellarRelayMessage::Data {
							p_id: _,
							msg_type: _,
							msg,
						} => match msg {
							StellarMessage::ScpMessage(env) => {
								self.collector
									.handle_envelope(env, &mut tx_set_hash_map, &overlay_conn)
									.await?;
							}
							StellarMessage::TxSet(set) => {
								self.collector.handle_tx_set(&set, &mut tx_set_hash_map, &self.tx_env_filters)?;
							}
							_ => {}
						},

						_ => {}
					}
				}
				Some(msg) = self.receiver.recv() => {
						self.handle_message(msg).await;
					}
			}
		}
	}
}


/// Handler to communicate with the ScpMessageActor
pub struct ScpMessageHandler {
	sender: mpsc::Sender<ActorMessage>,
}

impl ScpMessageHandler {
	/// creates a new Handler.
	fn new(
		overlay_conn: StellarOverlayConnection,
		vault_addresses: Vec<String>,
		is_public_network: bool,
	) -> Self {
		let collector = ScpMessageCollector::new(is_public_network, vault_addresses);

		let (sender, receiver) = mpsc::channel(1024);

		let mut actor = ScpMessageActor::new(receiver, collector);
		tokio::spawn(async move { actor.run(overlay_conn).await });

		Self { sender }
	}

	/// A sample method to communicate with the actor.
	/// Returns the size of the EnvelopesMap of the ScpMessageCollector.
	pub async fn get_size(&self) -> Result<usize, Error> {
		let (sender, receiver) = oneshot::channel();

		self.sender.send(ActorMessage::CurrentMapSize { sender }).await?;

		receiver.await.map_err(Error::from)
	}

	/// Adds a filter on what transactions to process.
	/// Returns an index of the filter in the map.
	pub async fn add_filter(
		&self,
		filter: Box<TxEnvelopeFilter>,
	) -> Result<(), Error> {
		tracing::info!("adding filter: {}", filter.id());
		self.sender.send(ActorMessage::AddFilter{ filter }).await.map_err(Error::from)
	}

	/// Removes an existing filter based on its id/key in the map.
	pub async fn remove_filter(&self, filter_id: u32) -> Result<(), Error> {
		self.sender.send(ActorMessage::RemoveFilter(filter_id)).await.map_err(Error::from)
	}

	/// Returns a list of transactions with each of their corresponding proofs
	pub async fn get_pending_txs(
		&self
	) -> Result<Vec<EncodedProof>, Error> {
		let (sender, receiver) = oneshot::channel();

		self.sender.send(ActorMessage::GetPendingTx { sender }).await?;

		receiver.await.map_err(Error::from)
	}

	pub fn handle_issue_event(&self) {
		todo!();
	}

	pub fn handle_redeem_event(&self) {
		todo!();
	}

}

/// Creates the ScpMessageHandler and contains the thread that connects and listens to the Stellar Node
///
/// # Arguments
///
/// * `node_info` - Information (w/o the address and port) of the Stellar Node to connect to.
/// * `connection_cfg` - The configuration on how and what (address and port) Stellar Node to connect to.
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
