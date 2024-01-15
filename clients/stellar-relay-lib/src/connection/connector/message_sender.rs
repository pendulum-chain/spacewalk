use std::time::Duration;
use substrate_stellar_sdk::types::{MessageType, SendMore, StellarMessage};
use tokio::{io::AsyncWriteExt, time::timeout};
use tokio::io::Interest;

use crate::connection::{
	flow_controller::MAX_FLOOD_MSG_CAP,
	handshake::create_auth_message,
	helper::{time_now, to_base64_xdr_string},
	Connector, Error,
};

impl Connector {
	pub async fn send_to_node(&mut self, msg: StellarMessage) -> Result<(), Error> {
		let xdr_msg = &self.create_xdr_message(msg)?;

		match timeout(
			Duration::from_secs(self.timeout_in_secs),
			self.write_stream_overlay.write_all(&xdr_msg),
		)
		.await
		{
			Ok(res) => {
				let result = res.map_err(|e| {
					log::error!("send_to_node(): Failed to send message to node: {e:?}");
					Error::WriteFailed(e.to_string())
				});

				let ready_status = self.write_stream_overlay.ready(Interest::WRITABLE | Interest::READABLE)
					.await.map_err(|e| {
						log::error!("send_to_node(): Stream not ready for reading or writing: {e:?}");
						Error::ConnectionFailed(e.to_string())
				})?;

				if ready_status.is_readable() {
					log::trace!("send_to_node(): stream is readable");
				}

				if ready_status.is_writable() {
					log::trace!("send_to_node(): stream is writable");
				}

				if ready_status.is_empty() {
					log::trace!("send_to_node(): stream is empty");
				}

				result
			},
			Err(_) => Err(Error::Timeout),
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
