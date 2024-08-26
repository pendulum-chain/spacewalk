use async_std::io::WriteExt;
use std::time::Duration;
use substrate_stellar_sdk::types::{MessageType, StellarMessage};
use tokio::time::timeout;
use tracing::debug;

use crate::connection::{
	handshake::create_auth_message,
	helper::{time_now, to_base64_xdr_string},
	Connector, Error,
};

impl Connector {
	pub async fn send_to_node(&mut self, msg: StellarMessage) -> Result<(), Error> {
		// Create the XDR message outside the closure
		let xdr_msg = self.create_xdr_message(msg)?;

		match timeout(
			Duration::from_secs(self.timeout_in_secs),
			self.tcp_stream.write_all(&xdr_msg),
		)
		.await
		{
			Ok(res) => res.map_err(|e| Error::WriteFailed(e.to_string())),
			Err(_) => Err(Error::Timeout),
		}
	}

	pub async fn send_hello_message(&mut self) -> Result<(), Error> {
		let msg = self.create_hello_message(time_now())?;
		debug!("send_hello_message(): Sending Hello Message: {}", to_base64_xdr_string(&msg));

		self.send_to_node(msg).await
	}

	pub(super) async fn send_auth_message(&mut self) -> Result<(), Error> {
		let msg = create_auth_message();
		debug!("send_auth_message(): Sending Auth Message: {}", to_base64_xdr_string(&msg));

		return self.send_to_node(create_auth_message()).await
	}

	pub(super) async fn check_to_send_more(
		&mut self,
		message_type: MessageType,
		data_len: usize
	) -> Result<(), Error> {

		let msg = self.flow_controller.send_more(message_type, data_len);
		if let Some(inner_msg) = msg {
			return self.send_to_node(inner_msg).await
		};
		Ok(())

	}
}
