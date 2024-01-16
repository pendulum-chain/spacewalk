use std::io::{Read, Write};
use substrate_stellar_sdk::types::{ErrorCode, StellarMessage};
use tokio::sync::{
	mpsc,
	mpsc::{
		error::{SendError, TryRecvError},
		Sender,
	},
};

use crate::{
	connection::{poll_messages_from_stellar, ConnectionInfo, Connector},
	helper::{create_stream, error_to_string},
	node::NodeInfo,
	Error,
};
use crate::helper::time_now;
use crate::xdr_converter::get_xdr_message_length;

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
		log::info!("connect(): connecting to {conn_info:?}");

		// this is a channel to communicate with the user/caller.
		let (send_to_user_sender, send_to_user_receiver) = mpsc::channel::<StellarMessage>(1024);

		let (send_to_node_sender, send_to_node_receiver) = mpsc::channel::<StellarMessage>(1024);

		// split the stream for easy handling of read and write
		let ((read_stream_overlay, write_stream_overlay), mut net_stream) =
			create_stream(&conn_info.address()).await?;

		let mut connector = Connector::new(local_node_info, conn_info, write_stream_overlay);

		let hello = connector.create_hello_message(time_now())?;
		let hello = connector.create_xdr_message(hello)?;

		let size = net_stream.write(&hello).map_err(|e| {
			log::error!("connect(): Failed to send hello message to net tcpstream: {e:?}");
			Error::WriteFailed(e.to_string())
		})?;
		log::trace!("connect(): sent hello message to net tcpstream: {size} bytes");

		let mut buffer = [0; 4];
		let _ = net_stream.read(&mut buffer[..])
			.map_err(|e| {
				log::error!("connect(): Failed to read from net tcpstream: {e:?}");
				Error::ReadFailed(e.to_string())
			})?;
		let xdr_msg_len = get_xdr_message_length(&buffer);
		log::trace!("connect(): taken from net tcp stream: the xdr msg len is {xdr_msg_len}");

		connector.send_hello_message().await?;

		tokio::spawn(poll_messages_from_stellar(
			connector,
			read_stream_overlay,
			send_to_user_sender,
			send_to_node_receiver,
		));

		Ok(StellarOverlayConnection {
			sender: send_to_node_sender,
			receiver: send_to_user_receiver,
		})
	}

	pub async fn listen(&mut self) -> Result<Option<StellarMessage>, Error> {
		loop {
			if !self.is_alive() {
				self.disconnect();
				return Err(Error::Disconnected)
			}

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

	pub fn is_alive(&mut self) -> bool {
		let is_closed = self.sender.is_closed();

		if is_closed {
			self.disconnect();
		}

		!is_closed
	}

	pub fn disconnect(&mut self) {
		log::info!("disconnect(): closing connection to overlay network");
		self.receiver.close();
	}
}

impl Drop for StellarOverlayConnection {
	fn drop(&mut self) {
		self.disconnect();
	}
}
