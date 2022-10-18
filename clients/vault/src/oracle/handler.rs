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
use crate::oracle::TxHandler;
use crate::oracle::types::TxSetCheckerMap;

pub struct NoFilter;

// Dummy filter that does nothing.
impl TxHandler<TransactionEnvelope> for NoFilter {
	fn process_tx(&self, _param: &TransactionEnvelope) -> bool {
		false
	}
}

pub enum Message {
	/// returns the envelopes map size.
	CurrentMapSize {
		sender: oneshot::Sender<usize>,
	},
	/// filters on the transaction we want to process
	FilterTx(Box<dyn TxHandler<TransactionEnvelope> + Send + Sync>),
	/// Gets all Pending Transactions with Proofs
	GetPendingTx {
		sender: oneshot::Sender<Vec<EncodedProof>>,
	},
}

/// Runs both the stellar-relay and its own.
struct ScpMessageActor {
	/// used to receive messages from outside the actor.
	receiver: mpsc::Receiver<Message>,
	collector: ScpMessageCollector,
	/// the filter used to filter out transactions that needs processing.
	tx_env_filter: Box<dyn TxHandler<TransactionEnvelope> + Send + Sync>,
}

impl ScpMessageActor {
	fn new(receiver: mpsc::Receiver<Message>, collector: ScpMessageCollector) -> Self {
		ScpMessageActor { receiver, collector, tx_env_filter: Box::new(NoFilter) }
	}

	/// handles messages sent from the outside.
	async fn handle_message(&mut self, msg: Message) {
		match msg {
			Message::CurrentMapSize { sender } => {
				let _ = sender.send(self.collector.envelopes_map_len());
			},

			Message::FilterTx(filter) => {
				self.tx_env_filter = filter;
			},

			Message::GetPendingTx { sender } => {
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
								self.collector.handle_tx_set(&set, &mut tx_set_hash_map, self.tx_env_filter.as_ref()).await?;
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
	sender: mpsc::Sender<Message>,
}

impl ScpMessageHandler {
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

	pub async fn get_size(&self, sender: oneshot::Sender<usize>) -> Result<(), Error> {
		self.sender.send(Message::CurrentMapSize { sender }).await.map_err(Error::from)
	}

	pub async fn filter_tx(
		&self,
		filter: Box<dyn TxHandler<TransactionEnvelope> + Send + Sync>,
	) -> Result<(), Error> {
		self.sender.send(Message::FilterTx(filter)).await.map_err(Error::from)
	}

	pub async fn get_pending_txs(
		&self,
		sender: oneshot::Sender<Vec<EncodedProof>>,
	) -> Result<(), Error> {
		self.sender.send(Message::GetPendingTx { sender }).await.map_err(Error::from)
	}

	pub fn handle_issue_event(&self) {
		todo!();
	}

	pub fn handle_redeem_event(&self) {
		todo!();
	}
}

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
