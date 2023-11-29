mod config;
// mod connection;
mod connection;
pub mod node;
#[cfg(test)]
mod tests;

pub use substrate_stellar_sdk as sdk;
use substrate_stellar_sdk::types::StellarMessage;
use tokio::net::{tcp, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;


pub use config::{connect_to_stellar_overlay_network, StellarOverlayConfig};
pub use crate::connection::{Error, helper};


use crate::connection::{ConnectionInfo, Connector, poll_messages_from_stellar};
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

	pub async fn listen(&mut self) -> Option<StellarMessage> {
		self.receiver.recv().await
	}

	pub fn is_alive(&self) -> bool {
		!self.sender.is_closed()
	}

	pub fn disconnect(&mut self) {
		self.receiver.close();
	}
}


impl StellarOverlayConnection {
	/// Returns an `StellarOverlayConnection` when a connection to Stellar Node is successful.
	pub async fn connect(local_node_info: NodeInfo, conn_info: ConnectionInfo) -> Result<Self,Error> {
		log::info!("connect(): connecting to {conn_info:?}");

		// split the stream for easy handling of read and write
		let (rd, wr) = create_stream(&conn_info.address()).await?;

		// this is a channel to communicate with the user/caller.
		let (send_to_user_sender, send_to_user_receiver) =
			mpsc::channel::<StellarMessage>(1024);

		let (send_to_node_sender, send_to_node_receiver) =
			mpsc::channel::<StellarMessage>(1024);

		let mut connector = Connector::new(
			local_node_info,
			conn_info,
			wr,
		);
		connector.send_hello_message().await?;

		tokio::spawn(poll_messages_from_stellar(connector,rd,send_to_user_sender,send_to_node_receiver ));

		Ok(StellarOverlayConnection {
			sender: send_to_node_sender,
			receiver: send_to_user_receiver
		})
	}
}

async fn create_stream(
	address: &str,
) -> Result<(tcp::OwnedReadHalf, tcp::OwnedWriteHalf), Error> {
	let stream = TcpStream::connect(address)
		.await
		.map_err(|e| Error::ConnectionFailed(e.to_string()))?;

	Ok(stream.into_split())
}