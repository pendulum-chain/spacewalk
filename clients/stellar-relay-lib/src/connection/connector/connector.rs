use std::fmt::{Debug, Formatter};
use substrate_stellar_sdk::{
	types::{AuthenticatedMessageV0, Curve25519Public, HmacSha256Mac, MessageType},
	XdrCodec,
};
use substrate_stellar_sdk::types::StellarMessage;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

use crate::{
	connection::{
		authentication::{gen_shared_key, ConnectionAuth},
		flow_controller::FlowController,
		hmac::{verify_hmac, HMacKeys},
		ConnectionInfo, handshake::HandshakeState,
		Error
	},
	node::{LocalInfo, NodeInfo, RemoteInfo},
};
use crate::connection::connector::message_reader::read_message_from_stellar;
use crate::helper::create_stream;

pub struct Connector {
	local: LocalInfo,

	remote_info: Option<RemoteInfo>,
	hmac_keys: Option<HMacKeys>,

	pub(crate) connection_auth: ConnectionAuth,
	pub(crate) timeout_in_secs: u64,

	remote_called_us: bool,
	receive_tx_messages: bool,
	receive_scp_messages: bool,

	handshake_state: HandshakeState,
	flow_controller: FlowController,

	/// for writing xdr messages to stream.
	pub(crate) wr: OwnedWriteHalf,
}

impl Debug for Connector {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let is_hmac_keys_filled = self.hmac_keys.is_some();
		f.debug_struct("Connector")
			.field("local", &self.local)
			.field("remote", &self.remote_info)
			.field("hmac_keys_exist", &is_hmac_keys_filled)
			.field("connection_auth", &self.connection_auth)
			.field("timeout_in_secs", &self.timeout_in_secs)
			.field("receive_tx_messages", &self.receive_tx_messages)
			.field("receive_scp_messages", &self.receive_scp_messages)
			.field("handshake_state", &self.handshake_state)
			.field("flow_controller", &self.flow_controller)
			.finish()
	}
}

impl Connector {
	/// Verifies the AuthenticatedMessage, received from the Stellar Node
	pub(super) fn verify_auth(
		&self,
		auth_msg: &AuthenticatedMessageV0,
		body: &[u8],
	) -> Result<(), Error> {
		let remote_info = self.remote_info.as_ref().ok_or(Error::NoRemoteInfo)?;
		log::trace!(
			"verify_auth(): remote sequence: {}, auth message sequence: {}",
			remote_info.sequence(),
			auth_msg.sequence
		);

		let auth_msg_xdr = auth_msg.to_base64_xdr();
		let auth_msg_xdr =
			String::from_utf8(auth_msg_xdr.clone()).unwrap_or(format!("{:?}", auth_msg_xdr));

		log::debug!("verify_auth(): received auth message from Stellar Node: {auth_msg_xdr}");

		if remote_info.sequence() != auth_msg.sequence {
			// must be handled on main thread because workers could mix up order of messages.
			return Err(Error::InvalidSequenceNumber)
		}

		let keys = self.hmac_keys.as_ref().ok_or(Error::MissingHmacKeys)?;

		verify_hmac(body, &keys.receiving().mac, &auth_msg.mac.to_xdr())?;

		Ok(())
	}

	pub fn get_shared_key(&mut self, remote_pub_key_ecdh: &Curve25519Public) -> HmacSha256Mac {
		match self.connection_auth.shared_key(remote_pub_key_ecdh, !self.remote_called_us) {
			None => {
				// generate a new one when there's none.
				let new_shared_key = gen_shared_key(
					remote_pub_key_ecdh,
					self.connection_auth.secret_key_ecdh(),
					self.connection_auth.pub_key_ecdh(),
					!self.remote_called_us,
				);

				self.connection_auth.set_shared_key(
					remote_pub_key_ecdh,
					new_shared_key.clone(),
					!self.remote_called_us,
				);

				new_shared_key
			},

			Some(shared_key) => shared_key.clone(),
		}
	}

	pub fn new(
		local_node: NodeInfo,
		conn_info: ConnectionInfo,
		write_half_of_stream: OwnedWriteHalf,
	) -> Self {
		let connection_auth = ConnectionAuth::new(
			&local_node.network_id,
			conn_info.keypair(),
			conn_info.auth_cert_expiration,
		);

		Connector {
			local: LocalInfo::new(local_node),
			remote_info: None,
			hmac_keys: None,
			connection_auth,
			timeout_in_secs: conn_info.timeout_in_secs,
			remote_called_us: conn_info.remote_called_us,
			receive_tx_messages: conn_info.recv_tx_msgs,
			receive_scp_messages: conn_info.recv_scp_msgs,
			handshake_state: HandshakeState::Connecting,
			flow_controller: FlowController::default(),
			wr: write_half_of_stream,
		}
	}

	pub fn local(&self) -> &LocalInfo {
		&self.local
	}

	pub fn local_sequence(&self) -> u64 {
		self.local.sequence()
	}

	pub fn increment_local_sequence(&mut self) {
		self.local.increment_sequence();
	}

	pub fn remote(&self) -> Option<&RemoteInfo> {
		self.remote_info.as_ref()
	}

	pub fn set_remote(&mut self, value: RemoteInfo) {
		self.remote_info = Some(value);
	}

	pub fn increment_remote_sequence(&mut self) -> Result<(), Error> {
		self.remote_info
			.as_mut()
			.map(|remote| remote.increment_sequence())
			.ok_or(Error::NoRemoteInfo)
	}

	pub fn hmac_keys(&self) -> Option<&HMacKeys> {
		self.hmac_keys.as_ref()
	}

	pub fn set_hmac_keys(&mut self, value: HMacKeys) {
		self.hmac_keys = Some(value);
	}

	// Connection Auth

	pub fn remote_called_us(&self) -> bool {
		self.remote_called_us
	}

	pub fn receive_tx_messages(&self) -> bool {
		self.receive_tx_messages
	}

	pub fn receive_scp_messages(&self) -> bool {
		self.receive_scp_messages
	}

	pub fn is_handshake_created(&self) -> bool {
		self.handshake_state >= HandshakeState::GotHello
	}

	pub fn got_hello(&mut self) {
		self.handshake_state = HandshakeState::GotHello;
	}

	pub fn handshake_completed(&mut self) {
		self.handshake_state = HandshakeState::Completed;
	}

	pub fn get_handshake_state(&self) -> &HandshakeState {
		&self.handshake_state
	}

	pub fn inner_check_to_send_more(&mut self, msg_type: MessageType) -> bool {
		self.flow_controller.send_more(msg_type)
	}
	pub fn enable_flow_controller(
		&mut self,
		local_overlay_version: u32,
		remote_overlay_version: u32,
	) {
		self.flow_controller.enable(local_overlay_version, remote_overlay_version)
	}
}


/// Polls for messages coming from the Stellar Node and communicates it back to the user
///
/// # Arguments
/// * `connector` - contains the config and necessary info for connecting to Stellar Node
/// * `r_stream` - the read half of the stream that is connected to Stellar Node
/// * `send_to_user_sender` - sends message from Stellar to the user
/// * `send_to_node_receiver` - receives message from user and writes it to the write half of the stream.
pub(crate) async fn poll_messages_from_stellar(
	mut connector: Connector,
	mut r_stream: OwnedReadHalf,
	address: String,
	send_to_user_sender: mpsc::Sender<StellarMessage>,
	mut send_to_node_receiver: mpsc::Receiver<StellarMessage>
) {
	log::info!("poll_messages_from_stellar(): started.");

	loop {
		if send_to_user_sender.is_closed() {
			log::info!("poll_messages_from_stellar(): closing receiver during disconnection");
			// close this channel as communication to user was closed.
			break;
		}

		tokio::select! {
			result_msg = read_message_from_stellar(&mut r_stream, connector.timeout_in_secs) => match result_msg {
                Err(e) => {
                    log::error!("poll_messages_from_stellar(): {e:?}");
                    break;
                },
				Ok(xdr) =>  match connector.process_raw_message(xdr).await {
                    Ok(Some(stellar_msg)) =>
                    // push message to user
                    if let Err(e) = send_to_user_sender.send(stellar_msg).await {
                        log::warn!("poll_messages_from_stellar(): Error occurred during sending message to user: {e:?}");
                    },
                    Ok(_) => log::info!("poll_messages_from_stellar(): no message for user"),
                    Err(e) => {
                        log::error!("poll_messages_from_stellar(): Error occurred during processing xdr message: {e:?}");
                        break;
                    }
                }
			},

			// push message to Stellar Node
			Some(msg) = send_to_node_receiver.recv() => if let Err(e) = connector.send_to_node(msg).await {
                log::error!("poll_messages_from_stellar(): Error occurred during sending message to node: {e:?}");
            }
		}
	}
	// make sure to drop/shutdown the stream
	connector.wr.forget();
	drop(r_stream);

	send_to_node_receiver.close();
	drop(send_to_user_sender);

	log::info!("poll_messages_from_stellar(): stopped.");
}
