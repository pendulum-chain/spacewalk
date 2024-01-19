use std::io::Write;
use substrate_stellar_sdk::types::{MessageType, SendMore, StellarMessage};

use crate::connection::{
	flow_controller::MAX_FLOOD_MSG_CAP,
	handshake::create_auth_message,
	helper::{time_now, to_base64_xdr_string},
	Connector, Error,
};

impl Connector {
	pub async fn send_to_node(&mut self, msg: StellarMessage) -> Result<(), Error> {
		// Create the XDR message outside the closure
		let xdr_msg = self.create_xdr_message(msg)?;

		// Clone the TcpStream (or its Arc<Mutex<_>> wrapper)
		let stream_clone = self.tcp_stream.clone();

		// this may really not be necessary
		let write_result = tokio::task::spawn_blocking(move || {
			let mut stream = stream_clone.lock().unwrap();
			stream.write_all(&xdr_msg).map_err(|e| {
				log::error!("send_to_node(): Failed to send message to node: {e:?}");
				Error::WriteFailed(e.to_string())
			})
		});

		// Await the result of the blocking task
		match write_result.await {
			Ok(result) => result,
			Err(e) => {
				log::error!("send_to_node(): Error occurred in blocking task: {e:?}");
				Err(Error::WriteFailed(e.to_string()))
			},
		}
	}

	pub async fn send_hello_message(&mut self) -> Result<(), Error> {
		let msg = self.create_hello_message(time_now())?;
		log::info!("send_hello_message(): Sending Hello Message: {}", to_base64_xdr_string(&msg));

		self.send_to_node(msg).await
	}

	pub(super) async fn send_auth_message(&mut self) -> Result<(), Error> {
		let msg = create_auth_message();
		log::info!("send_auth_message(): Sending Auth Message: {}", to_base64_xdr_string(&msg));

		self.send_to_node(create_auth_message()).await
	}

	pub(super) async fn check_to_send_more(
		&mut self,
		message_type: MessageType,
	) -> Result<(), Error> {
		if !self.inner_check_to_send_more(message_type) {
			return Ok(())
		}

		let msg = StellarMessage::SendMore(SendMore { num_messages: MAX_FLOOD_MSG_CAP });
		self.send_to_node(msg).await
	}
}
