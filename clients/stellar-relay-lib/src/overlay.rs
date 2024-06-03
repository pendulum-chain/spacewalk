use substrate_stellar_sdk::types::{ErrorCode, StellarMessage};
use tokio::sync::{
	mpsc,
	mpsc::{
		error::{SendError, TryRecvError},
		Sender,
	},
};
use tracing::{error, info};

use crate::{
	connection::{poll_messages_from_stellar, ConnectionInfo, Connector},
	helper::error_to_string,
	node::NodeInfo,
	Error,
};

/// Used to send/receive messages to/from Stellar Node
pub struct StellarOverlayConnection {
	sender: mpsc::Sender<StellarMessage>,
	receiver: mpsc::Receiver<StellarMessage>,
}

impl StellarOverlayConnection {
	pub fn sender(&self) -> Sender<StellarMessage> {
		self.sender.clone()
	}

	pub async fn send_to_node(&self, msg: StellarMessage) -> Result<(), SendError<StellarMessage>> {
		self.sender.send(msg).await
	}

	/// Returns an `StellarOverlayConnection` when a connection to Stellar Node is successful.
	pub async fn connect(
		local_node_info: NodeInfo,
		conn_info: ConnectionInfo,
	) -> Result<Self, Error> {
		info!("connect(): connecting to {conn_info:?}");

		// this is a channel to communicate with the user/caller.
		let (send_to_user_sender, send_to_user_receiver) = mpsc::channel::<StellarMessage>(1024);

		let (send_to_node_sender, send_to_node_receiver) = mpsc::channel::<StellarMessage>(1024);

		let connector = Connector::start(local_node_info, conn_info).await?;

		tokio::spawn(poll_messages_from_stellar(
			connector,
			send_to_user_sender,
			send_to_node_receiver,
		));

		Ok(StellarOverlayConnection {
			sender: send_to_node_sender,
			receiver: send_to_user_receiver,
		})
	}

	pub fn listen(&mut self) -> Result<Option<StellarMessage>, Error> {
		loop {
			if !self.is_alive() {
				return Err(Error::Disconnected)
			}

			match self.receiver.try_recv() {
				Ok(StellarMessage::ErrorMsg(e)) => {
					error!("listen(): received error message: {e:?}");
					if e.code == ErrorCode::ErrConf || e.code == ErrorCode::ErrAuth {
						return Err(Error::ConnectionFailed(error_to_string(e)))
					}

					return Ok(None)
				},
				Ok(msg) => return Ok(Some(msg)),
				Err(TryRecvError::Disconnected) => return Err(Error::Disconnected),
				Err(TryRecvError::Empty) => continue,
			}
		}
	}

	pub fn is_alive(&mut self) -> bool {
		let is_closed = self.sender.is_closed();

		if is_closed {
			self.stop();
		}

		!is_closed
	}

	pub fn stop(&mut self) {
		info!("stop(): closing connection to overlay network");
		self.receiver.close();
	}
}

impl Drop for StellarOverlayConnection {
	fn drop(&mut self) {
		self.stop();
	}
}
