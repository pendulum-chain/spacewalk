use crate::{
	connection::{
		connector::ConnectorActions,
		services::{connection_handler, create_stream, receiving_service},
	},
	node::NodeInfo,
	ConnConfig, Connector, Error, StellarRelayMessage,
};
use substrate_stellar_sdk::types::StellarMessage;
use tokio::{sync::mpsc, time::Duration};

pub struct StellarOverlayConnection {
	/// This is when we want to send stellar messages
	tx: mpsc::Sender<ConnectorActions>,
	/// For receiving stellar messages
	rx: mpsc::Receiver<StellarRelayMessage>,
	local_node: NodeInfo,
	cfg: ConnConfig,
	max_retries: u8,
}

impl StellarOverlayConnection {
	fn new(
		tx: mpsc::Sender<ConnectorActions>,
		rx: mpsc::Receiver<StellarRelayMessage>,
		max_retries: u8,
		local_node: NodeInfo,
		cfg: ConnConfig,
	) -> Self {
		StellarOverlayConnection { tx, rx, local_node, cfg, max_retries }
	}

	pub async fn send(&self, message: StellarMessage) -> Result<(), Error> {
		self.tx.send(ConnectorActions::SendMessage(message)).await.map_err(Error::from)
	}

	/// Receives Stellar messages from the connection.
	/// Restarts the connection when lost.
	pub async fn listen(&mut self) -> Option<StellarRelayMessage> {
		let res = self.rx.recv().await;
		if let Some(StellarRelayMessage::Timeout) = &res {
			while self.max_retries > 0 {
				log::info!("reconnecting to {:?}.", &self.cfg.address);
				if let Ok(new_user) =
					StellarOverlayConnection::connect(self.local_node.clone(), self.cfg.clone()).await
				{
					self.max_retries = new_user.max_retries;
					self.tx = new_user.tx;
					self.rx = new_user.rx;
					return self.rx.recv().await
				} else {
					self.max_retries -= 1;
					log::error!(
						"failed to reconnect! # of retries left: {}. Retrying in 3 seconds...",
						self.max_retries
					);
					tokio::time::sleep(Duration::from_secs(3)).await;
				}
			}
		}
		res
	}

	/// Triggers connection to the Stellar Node.
	/// Returns the UserControls for the user to send and receive Stellar messages.
	pub async fn connect(local_node: NodeInfo, cfg: ConnConfig) -> Result<StellarOverlayConnection, Error> {
		let retries = cfg.retries;
		let timeout_in_secs = cfg.timeout_in_secs;
		// split the stream for easy handling of read and write
		let (rd, wr) = create_stream(&cfg.address()).await?;
		// ------------------ prepare the channels
		// this is a channel to communicate with the connection/config (this needs renaming)
		let (actions_sender, actions_receiver) = mpsc::channel::<ConnectorActions>(1024);
		// this is a channel to communicate with the user/caller.
		let (message_writer, message_receiver) = mpsc::channel::<StellarRelayMessage>(1024);
		let overlay_connection = StellarOverlayConnection::new(
			actions_sender.clone(),
			message_receiver,
			cfg.retries,
			local_node,
			cfg,
		);
		let conn = Connector::new(
			overlay_connection.local_node.clone(),
			overlay_connection.cfg.clone(),
			actions_sender.clone(),
			message_writer,
		);
		// start the receiving_service
		tokio::spawn(receiving_service(rd, actions_sender.clone(), timeout_in_secs, retries));
		// run the conn communication
		tokio::spawn(connection_handler(conn, actions_receiver, wr));
		// start the handshake
		actions_sender.send(ConnectorActions::SendHello).await?;
		Ok(overlay_connection)
	}
}
