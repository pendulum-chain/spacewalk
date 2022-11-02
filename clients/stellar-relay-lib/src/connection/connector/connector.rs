use substrate_stellar_sdk::{
	types::{AuthenticatedMessageV0, Curve25519Public, HmacSha256Mac, MessageType},
	XdrCodec,
};
use tokio::sync::mpsc;

use crate::{
	connection::{
		authentication::{gen_shared_key, ConnectionAuth},
		flow_controller::FlowController,
		hmac::{verify_hmac, HMacKeys},
	},
	handshake::HandshakeState,
	node::{LocalInfo, NodeInfo, RemoteInfo},
	ConnConfig, ConnectorActions, Error, StellarRelayMessage,
};

pub struct Connector {
	local: LocalInfo,

	remote_info: Option<RemoteInfo>,
	hmac_keys: Option<HMacKeys>,

	pub(crate) connection_auth: ConnectionAuth,
	pub(crate) timeout_in_secs: u64,
	pub(crate) retries: u8,

	remote_called_us: bool,
	receive_tx_messages: bool,
	receive_scp_messages: bool,

	handshake_state: HandshakeState,
	flow_controller: FlowController,

	/// a channel for writing xdr messages to stream.
	actions_sender: mpsc::Sender<ConnectorActions>,

	/// a channel for communicating back to the caller
	relay_message_sender: mpsc::Sender<StellarRelayMessage>,
}

impl Connector {
	/// Verifies the AuthenticatedMessage, received from the Stellar Node
	pub(crate) fn verify_auth(
		&self,
		auth_msg: &AuthenticatedMessageV0,
		body: &[u8],
	) -> Result<(), Error> {
		let remote_info = self.remote_info.as_ref().ok_or(Error::NoRemoteInfo)?;
		log::debug!(
			"remote sequence: {}, auth message sequence: {}",
			remote_info.sequence(),
			auth_msg.sequence
		);
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
		cfg: ConnConfig,
		actions_sender: mpsc::Sender<ConnectorActions>,
		relay_message_sender: mpsc::Sender<StellarRelayMessage>,
	) -> Self {
		let connection_auth =
			ConnectionAuth::new(&local_node.network_id, cfg.keypair(), cfg.auth_cert_expiration);

		Connector {
			local: LocalInfo::new(local_node),
			remote_info: None,
			hmac_keys: None,
			connection_auth,
			timeout_in_secs: cfg.timeout_in_secs,
			retries: cfg.retries,
			remote_called_us: cfg.remote_called_us,
			receive_tx_messages: cfg.recv_scp_messages,
			receive_scp_messages: cfg.recv_scp_messages,
			handshake_state: HandshakeState::Connecting,
			flow_controller: FlowController::default(),
			actions_sender,
			relay_message_sender,
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

	pub async fn send_to_user(&self, msg: StellarRelayMessage) -> Result<(), Error> {
		self.relay_message_sender.send(msg).await.map_err(Error::from)
	}

	pub async fn send_to_node(&self, action: ConnectorActions) -> Result<(), Error> {
		self.actions_sender.send(action).await.map_err(Error::from)
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
