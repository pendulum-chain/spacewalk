use std::vec;

use substrate_stellar_sdk::{
	types::{AuthenticatedMessageV0, Curve25519Public, HmacSha256Mac, MessageType, Hello},
	XdrCodec, compound_types::LimitedString, PublicKey,
};
use tokio::sync::mpsc;

use crate::{
	connection::{
		authentication::{gen_shared_key, ConnectionAuth, create_auth_cert},
		flow_controller::FlowController,
		hmac::{verify_hmac, HMacKeys},
	},
	handshake::HandshakeState,
	node::{LocalInfo, NodeInfo, RemoteInfo},
	ConnConfig, ConnectorActions, Error, StellarRelayMessage, helper::time_now,
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


mod test{
	use crate::Connector;
	use std::vec;
	use substrate_stellar_sdk::{
		types::{AuthenticatedMessageV0, Curve25519Public, HmacSha256Mac, MessageType, Hello},
		XdrCodec, compound_types::LimitedString, PublicKey,
	};
	use tokio::sync::mpsc;

	use crate::{
		connection::{
			authentication::{gen_shared_key, ConnectionAuth, create_auth_cert},
			flow_controller::FlowController,
			hmac::{verify_hmac, HMacKeys},
		},
		handshake::HandshakeState,
		node::{LocalInfo, NodeInfo, RemoteInfo},
		ConnConfig, ConnectorActions, Error, StellarRelayMessage, helper::time_now,
	};
	#[test]
	fn create_new_connector_works() {
		use substrate_stellar_sdk::{
			network::TEST_NETWORK,
		};
		let (node_info, _, mut connector) = create_connector();
	
		let connector_local_node = connector.local.node();
		
		assert_eq!(connector_local_node.ledger_version, node_info.ledger_version);
		assert_eq!(connector_local_node.overlay_version, node_info.overlay_version);
		assert_eq!(connector_local_node.overlay_min_version, node_info.overlay_min_version);
		assert_eq!(connector_local_node.version_str, node_info.version_str);
		assert_eq!(connector_local_node.network_id, node_info.network_id);
	}
	
	#[test]
	fn connector_local_sequence_works() {
		let (node_info, _, mut connector) = create_connector();
		assert_eq!(connector.local_sequence(), 0);
		connector.increment_local_sequence();
		assert_eq!(connector.local_sequence(), 1);
	}
	
	#[test]
	fn connector_set_remote_works() {
		let (node_info, _, mut connector) = create_connector();
	
		let connector_auth = &connector.connection_auth;
		let new_auth_cert =
		create_auth_cert_from_connection_auth(connector_auth);
	
		let hello = Hello{ledger_version : 0, overlay_version: 0, overlay_min_version: 0, network_id: [0;32], version_str: LimitedString::<100_i32>::new(vec![]).unwrap(), listening_port: 11625, peer_id: PublicKey::PublicKeyTypeEd25519([0;32]), cert: new_auth_cert, nonce: [0;32] };
		connector.set_remote(RemoteInfo::new(&hello) );
	
		assert!(connector.remote().is_some());
	}
	
	#[test]
	fn connector_increment_remote_sequence_works() {
		let (node_info, _, mut connector) = create_connector();
		
	
		let connector_auth = &connector.connection_auth;
		let new_auth_cert = create_auth_cert_from_connection_auth(connector_auth);
	
		let hello = Hello{ledger_version : 0, overlay_version: 0, overlay_min_version: 0, network_id: [0;32], version_str: LimitedString::<100_i32>::new(vec![]).unwrap(), listening_port: 11625, peer_id: PublicKey::PublicKeyTypeEd25519([0;32]), cert: new_auth_cert, nonce: [0;32] };
		connector.set_remote(RemoteInfo::new(&hello) );
		assert_eq!(connector.remote().unwrap().sequence(), 0);
	
		connector.increment_remote_sequence().unwrap();
		connector.increment_remote_sequence().unwrap();
		connector.increment_remote_sequence().unwrap();
		assert_eq!(connector.remote().unwrap().sequence(), 3);
	}

	#[test]
	fn connector_get_set_hmac_keys_works() {

		//arrange
		let (node_info, _, mut connector) = create_connector();
		let connector_auth = &connector.connection_auth;
		let new_auth_cert = create_auth_cert_from_connection_auth(connector_auth);
	
		let hello = Hello{ledger_version : 0, overlay_version: 0, overlay_min_version: 0, network_id: [0;32], version_str: LimitedString::<100_i32>::new(vec![]).unwrap(), listening_port: 11625, peer_id: PublicKey::PublicKeyTypeEd25519([0;32]), cert: new_auth_cert, nonce: [0;32] };
		let remote = RemoteInfo::new(&hello);
		let remote_nonce = remote.nonce();
		connector.set_remote(remote.clone());

		let shared_key = connector.get_shared_key(&remote.pub_key_ecdh());
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

	

	fn create_auth_cert_from_connection_auth(connector_auth: &ConnectionAuth) -> substrate_stellar_sdk::types::AuthCert {
		let time_now = time_now();
		let new_auth_cert =
					create_auth_cert(connector_auth.network_id(), connector_auth.keypair(), time_now, connector_auth.pub_key_ecdh().clone())
						.expect("should successfully create an auth cert");
		new_auth_cert
	}
	
	fn create_connector() -> (NodeInfo, ConnConfig, Connector) {
		use substrate_stellar_sdk::{
			network::TEST_NETWORK, SecretKey,
		};
		let secret =
				SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
					.unwrap();
		let node_info = NodeInfo::new(19, 21, 19, "v19.1.0".to_string(), &TEST_NETWORK);
		let cfg = ConnConfig::new("34.235.168.98", 11625, secret, 0, false, true, false);
		// this is a channel to communicate with the connection/config (this needs renaming)
		let (actions_sender, _) = mpsc::channel::<ConnectorActions>(1024);
		// this is a channel to communicate with the user/caller.
		let (relay_message_sender, _) = mpsc::channel::<StellarRelayMessage>(1024);
		let mut connector = Connector::new(
				node_info.clone(),
				cfg.clone(),
				actions_sender.clone(),
				relay_message_sender,
			);
		(node_info, cfg, connector)
	}
	
}
