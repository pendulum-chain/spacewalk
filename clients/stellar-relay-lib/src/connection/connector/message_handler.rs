use crate::{
	connection::{
		authentication::verify_remote_auth_cert,
		helper::{error_to_string, time_now},
		hmac::HMacKeys,
		xdr_converter::parse_authenticated_message,
		Connector, Error, Xdr,
	},
	node::RemoteInfo,
};
use substrate_stellar_sdk::{
	types::{ErrorCode, Hello, MessageType, StellarMessage},
	XdrCodec,
};
use tracing::{error, info, trace, warn};

impl Connector {
	/// Processes the raw bytes from the stream
	pub(super) async fn process_raw_message(
		&mut self,
		data: Xdr,
	) -> Result<Option<StellarMessage>, Error> {
		let (auth_msg, msg_type) = parse_authenticated_message(&data)?;

		match msg_type {
			MessageType::Transaction | MessageType::FloodAdvert if !self.receive_tx_messages() => {
				self.increment_remote_sequence()?;
				self.maybe_reclaim_capacity(MessageType::Transaction, data.len()).await?;
			},

			MessageType::ScpMessage if !self.receive_scp_messages() => {
				self.increment_remote_sequence()?;
			},

			MessageType::ErrorMsg => match auth_msg.message {
				StellarMessage::ErrorMsg(e) => {
					error!(
						"process_raw_message(): Received ErrorMsg during authentication: {}",
						error_to_string(e.clone())
					);
					return Err(Error::from(e));
				},
				other => error!(
					"process_raw_message(): Received a different message during authentication: {:?}",
					other
				),
			},

			_ => {
				// we only verify the authenticated message when a handshake has been done.
				if self.is_handshake_created() {
					self.verify_auth(&auth_msg, &data[4..(data.len() - 32)])?;
					self.increment_remote_sequence()?;
					trace!("process_raw_message(): Processing {msg_type:?} message: auth verified");
				}
				return self.process_stellar_message(auth_msg.message, msg_type, data.len()).await;
			},
		}
		Ok(None)
	}

	/// Returns a StellarMessage for the user/outsider. Else none if user/outsider do not need it.
	/// This handles what to do with the Stellar message.
	async fn process_stellar_message(
		&mut self,
		msg: StellarMessage,
		msg_type: MessageType,
		data_len: usize,
	) -> Result<Option<StellarMessage>, Error> {
		match msg {
			StellarMessage::Hello(hello) => {
				// update the node info based on the hello message
				self.process_hello_message(hello.clone())?;

				self.got_hello();

				if self.remote_called_us() {
					self.send_hello_message().await?;
				} else {
					self.send_auth_message(self.local().node().overlay_version).await?;
				}
				info!("process_stellar_message(): Hello message processed successfully");
				// Pass the hello message to the user/outsider. To signal that the Overlay is ready.
				return Ok(Some(StellarMessage::Hello(hello)));
			},

			StellarMessage::Auth(_) => {
				self.process_auth_message().await?;
			},

			StellarMessage::ErrorMsg(e) => {
				error!("process_stellar_message(): Received ErrorMsg during authentication: {e:?}");
				if e.code == ErrorCode::ErrConf || e.code == ErrorCode::ErrAuth {
					return Err(Error::from(e));
				}
				return Ok(Some(StellarMessage::ErrorMsg(e)));
			},
			StellarMessage::SendMore(_) => {},
			StellarMessage::SendMoreExtended(_) => {},
			// we do not handle other messages. Return to caller
			other => {
				self.maybe_reclaim_capacity(msg_type, data_len).await?;
				return Ok(Some(other));
			},
		}

		Ok(None)
	}

	async fn process_auth_message(&mut self) -> Result<(), Error> {
		if self.remote_called_us() {
			self.send_auth_message(self.local().node().overlay_version).await?;
		}

		self.handshake_completed();

		if let Some(remote) = self.remote() {
			let msg = self.maybe_start_flow_control_bytes(
				self.local().node().overlay_version,
				remote.node().overlay_version,
			);
			self.send_to_node(msg).await?;
		} else {
			warn!("process_auth_message(): No remote overlay version after handshake.");
		}
		Ok(())
	}

	/// Updates the config based on the hello message that was received from the Stellar Node
	fn process_hello_message(&mut self, hello: Hello) -> Result<(), Error> {
		let mut network_id = self.connection_auth.network_id().to_xdr();

		if !verify_remote_auth_cert(time_now(), &hello.peer_id, &hello.cert, &mut network_id) {
			return Err(Error::AuthCertInvalid);
		}

		let remote_info = RemoteInfo::new(hello);
		let shared_key = self.get_shared_key(remote_info.pub_key_ecdh());

		self.set_hmac_keys(HMacKeys::new(
			&shared_key,
			self.local().nonce(),
			remote_info.nonce(),
			self.remote_called_us(),
		));

		self.set_remote(remote_info);

		Ok(())
	}
}
