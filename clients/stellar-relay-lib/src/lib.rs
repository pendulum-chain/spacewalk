mod config;
// mod connection;
mod connection;
pub mod node;
#[cfg(test)]
mod tests;

pub use substrate_stellar_sdk as sdk;
use substrate_stellar_sdk::types::{ErrorCode, StellarMessage};
use tokio::net::{tcp, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::{SendError, TryRecvError};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;

pub use config::{connect_to_stellar_overlay_network, StellarOverlayConfig};
pub use crate::connection::{Error, helper};


use crate::connection::{ConnectionInfo, Connector, poll_messages_from_stellar};
use crate::helper::{create_stream, error_to_string};
use crate::node::NodeInfo;

/// Used to send/receive messages to/from Stellar Node
pub struct StellarOverlayConnection {
	sender: mpsc::Sender<StellarMessage>,
	receiver: mpsc::Receiver<StellarMessage>
}

impl StellarOverlayConnection {
	pub async fn send_to_node(&self, msg:StellarMessage) -> Result<(), SendError<StellarMessage>> {
		self.sender.send(msg).await
	}

	pub async fn listen(&mut self) -> Result<Option<StellarMessage>, Error> {
		loop {
			match self.receiver.try_recv() {
				Ok(StellarMessage::ErrorMsg(e)) => {
					log::error!("listen(): received error message: {e:?}");
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

	pub fn is_alive(&self) -> bool {
		let result = self.sender.is_closed();

		if result {
			drop(self);
		}

		!result
	}

	pub fn disconnect(&mut self) {
		log::info!("disconnect(): closing channel");
		self.receiver.close();
	}
}

impl Drop for StellarOverlayConnection {
	fn drop(&mut self) {
		self.disconnect();
	}
}

impl StellarOverlayConnection {
	/// Returns an `StellarOverlayConnection` when a connection to Stellar Node is successful.
	pub async fn connect(local_node_info: NodeInfo, conn_info: ConnectionInfo) -> Result<Self,Error> {
		log::info!("connect(): connecting to {conn_info:?}");


		// this is a channel to communicate with the user/caller.
		let (send_to_user_sender, send_to_user_receiver) =
			mpsc::channel::<StellarMessage>(1024);

		let (send_to_node_sender, send_to_node_receiver) =
			mpsc::channel::<StellarMessage>(1024);

		// split the stream for easy handling of read and write
		let (rd, wr) = create_stream(&conn_info.address()).await?;

		let address = conn_info.address();
		let mut connector = Connector::new(
			local_node_info,
			conn_info,
			wr,
		);
		connector.send_hello_message().await?;

		tokio::spawn(poll_messages_from_stellar(connector,rd,address, send_to_user_sender,send_to_node_receiver));

		Ok(StellarOverlayConnection {
			sender: send_to_node_sender,
			receiver: send_to_user_receiver
		})
	}
}