use std::fmt::{Debug, Formatter};
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
	node::{LocalInfo, NodeInfo, RemoteInfo},
	ConnectionInfo, ConnectorActions, Error, HandshakeState, StellarRelayMessage,
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

impl Debug for Connector {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let is_hmac_keys_filled  = self.hmac_keys.is_some();
		f.debug_struct("Connector")
			.field("local", &self.local)
			.field("remote", &self.remote_info)
			.field("hmac_keys_exist",&is_hmac_keys_filled)
			.field("connection_auth", &self.connection_auth)
			.field("timeout_in_secs", &self.timeout_in_secs)
			.field("retries", &self.retries)
			.field("receive_tx_messages", &self.receive_tx_messages)
			.field("receive_scp_messages", &self.receive_scp_messages)
			.field("handshake_state", &self.handshake_state)
			.field("flow_controller", &self.flow_controller)
			.finish()
	}
}

impl Drop for Connector {
	fn drop(&mut self) {
		log::trace!("dropped Connector: {:?}",self);
	}
}

impl Connector {
	/// Verifies the AuthenticatedMessage, received from the Stellar Node
	pub(crate) fn verify_auth(
		&self,
		auth_msg: &AuthenticatedMessageV0,
		body: &[u8],
	) -> Result<(), Error> {
		let remote_info = self.remote_info.as_ref().ok_or(Error::NoRemoteInfo)?;
		log::trace!(
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
		conn_info: ConnectionInfo,
		actions_sender: mpsc::Sender<ConnectorActions>,
		relay_message_sender: mpsc::Sender<StellarRelayMessage>,
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
			retries: conn_info.retries,
			remote_called_us: conn_info.remote_called_us,
			receive_tx_messages: conn_info.recv_tx_msgs,
			receive_scp_messages: conn_info.recv_scp_msgs,
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

#[cfg(test)]
mod test {
	use crate::{connection::hmac::HMacKeys, node::RemoteInfo, Connector, StellarOverlayConfig};

	use substrate_stellar_sdk::{
		compound_types::LimitedString,
		types::{Hello, MessageType},
		PublicKey,
	};
	use tokio::sync::mpsc::{self, Receiver};

	use crate::{
		connection::authentication::{create_auth_cert, ConnectionAuth},
		helper::time_now,
		node::NodeInfo,
		ConnectionInfo, ConnectorActions, StellarRelayMessage,
	};

	#[cfg(test)]
	fn create_auth_cert_from_connection_auth(
		connector_auth: &ConnectionAuth,
	) -> substrate_stellar_sdk::types::AuthCert {
		let time_now = time_now();
		let new_auth_cert = create_auth_cert(
			connector_auth.network_id(),
			connector_auth.keypair(),
			time_now,
			connector_auth.pub_key_ecdh().clone(),
		)
		.expect("should successfully create an auth cert");
		new_auth_cert
	}

	#[cfg(test)]
	fn create_connector() -> (
		NodeInfo,
		ConnectionInfo,
		Connector,
		Receiver<ConnectorActions>,
		Receiver<StellarRelayMessage>,
	) {
		let cfg_file_path = "./resources/config/testnet/stellar_relay_config_sdftest1.json";
		let secret_key_path = "./resources/secretkey/stellar_secretkey_testnet";
		let secret_key =
			std::fs::read_to_string(secret_key_path).expect("should be able to read file");

		let cfg =
			StellarOverlayConfig::try_from_path(cfg_file_path).expect("should create a config");
		let node_info = cfg.node_info();
		let conn_info = cfg.connection_info(&secret_key).expect("should create a connection info");
		// this is a channel to communicate with the connection/config (this needs renaming)
		let (actions_sender, actions_receiver) = mpsc::channel::<ConnectorActions>(1024);
		// this is a channel to communicate with the user/caller.
		let (relay_message_sender, relay_message_receiver) =
			mpsc::channel::<StellarRelayMessage>(1024);
		let connector = Connector::new(
			node_info.clone(),
			conn_info.clone(),
			actions_sender,
			relay_message_sender,
		);
		(node_info, conn_info, connector, actions_receiver, relay_message_receiver)
	}

	#[test]
	fn create_new_connector_works() {
		let (node_info, _, connector, _, _) = create_connector();

		let connector_local_node = connector.local.node();

		assert_eq!(connector_local_node.ledger_version, node_info.ledger_version);
		assert_eq!(connector_local_node.overlay_version, node_info.overlay_version);
		assert_eq!(connector_local_node.overlay_min_version, node_info.overlay_min_version);
		assert_eq!(connector_local_node.version_str, node_info.version_str);
		assert_eq!(connector_local_node.network_id, node_info.network_id);
	}

	#[test]
	fn connector_local_sequence_works() {
		let (_node_info, _, mut connector, _, _) = create_connector();
		assert_eq!(connector.local_sequence(), 0);
		connector.increment_local_sequence();
		assert_eq!(connector.local_sequence(), 1);
	}

	#[test]
	fn connector_set_remote_works() {
		let (_node_info, _, mut connector, _, _) = create_connector();

		let connector_auth = &connector.connection_auth;
		let new_auth_cert = create_auth_cert_from_connection_auth(connector_auth);

		let hello = Hello {
			ledger_version: 0,
			overlay_version: 0,
			overlay_min_version: 0,
			network_id: [0; 32],
			version_str: LimitedString::<100_i32>::new(vec![]).unwrap(),
			listening_port: 11625,
			peer_id: PublicKey::PublicKeyTypeEd25519([0; 32]),
			cert: new_auth_cert,
			nonce: [0; 32],
		};
		connector.set_remote(RemoteInfo::new(&hello));

		assert!(connector.remote().is_some());
	}

	#[test]
	fn connector_increment_remote_sequence_works() {
		let (_node_info, _, mut connector, _, _) = create_connector();

		let connector_auth = &connector.connection_auth;
		let new_auth_cert = create_auth_cert_from_connection_auth(connector_auth);

		let hello = Hello {
			ledger_version: 0,
			overlay_version: 0,
			overlay_min_version: 0,
			network_id: [0; 32],
			version_str: LimitedString::<100_i32>::new(vec![]).unwrap(),
			listening_port: 11625,
			peer_id: PublicKey::PublicKeyTypeEd25519([0; 32]),
			cert: new_auth_cert,
			nonce: [0; 32],
		};
		connector.set_remote(RemoteInfo::new(&hello));
		assert_eq!(connector.remote().unwrap().sequence(), 0);

		connector.increment_remote_sequence().unwrap();
		connector.increment_remote_sequence().unwrap();
		connector.increment_remote_sequence().unwrap();
		assert_eq!(connector.remote().unwrap().sequence(), 3);
	}

	#[test]
	fn connector_get_and_set_hmac_keys_works() {
		//arrange
		let (_, _, mut connector, _, _) = create_connector();
		let connector_auth = &connector.connection_auth;
		let new_auth_cert = create_auth_cert_from_connection_auth(connector_auth);

		let hello = Hello {
			ledger_version: 0,
			overlay_version: 0,
			overlay_min_version: 0,
			network_id: [0; 32],
			version_str: LimitedString::<100_i32>::new(vec![]).unwrap(),
			listening_port: 11625,
			peer_id: PublicKey::PublicKeyTypeEd25519([0; 32]),
			cert: new_auth_cert,
			nonce: [0; 32],
		};
		let remote = RemoteInfo::new(&hello);
		let remote_nonce = remote.nonce();
		connector.set_remote(remote.clone());

		let shared_key = connector.get_shared_key(remote.pub_key_ecdh());
		assert!(connector.hmac_keys().is_none());
		//act
		connector.set_hmac_keys(HMacKeys::new(
			&shared_key,
			connector.local().nonce(),
			remote_nonce,
			connector.remote_called_us(),
		));
		//assert
		assert!(connector.hmac_keys().is_some());
	}

	#[test]
	fn connector_method_works() {
		let (_, conn_config, mut connector, _, _) = create_connector();

		assert_eq!(connector.remote_called_us(), conn_config.remote_called_us);
		assert_eq!(connector.receive_tx_messages(), conn_config.recv_tx_msgs);
		assert_eq!(connector.receive_scp_messages(), conn_config.recv_scp_msgs);

		connector.got_hello();
		assert!(connector.is_handshake_created());

		connector.handshake_completed();
		assert!(connector.is_handshake_created());
	}

	#[tokio::test]
	async fn connector_send_to_user_works() {
		let (_, _, connector, _, mut message_receiver) = create_connector();

		let message = StellarRelayMessage::Timeout;
		connector.send_to_user(message).await.unwrap();

		let received_message = message_receiver.recv().await;
		assert!(received_message.is_some());
		let message = received_message.unwrap();
		match message {
			StellarRelayMessage::Timeout => {},
			_ => {
				panic!("Incorrect message received!!!")
			},
		}
	}

	#[test]
	fn enable_flow_controller_works() {
		let (node_info, _, mut connector, _, _) = create_connector();

		assert!(!connector.inner_check_to_send_more(MessageType::ScpMessage));
		connector.enable_flow_controller(node_info.overlay_version, node_info.overlay_version);
	}

	#[tokio::test]
	async fn connector_send_to_node_works() {
		let (_, _, connector, mut actions_receiver, _) = create_connector();

		connector.send_to_node(ConnectorActions::SendHello).await.unwrap();

		let received_message = actions_receiver.recv().await;
		assert!(received_message.is_some());
		let message = received_message.unwrap();
		match message {
			ConnectorActions::SendHello => {},
			_ => {
				panic!("Incorrect message received!!!")
			},
		}
	}
}
