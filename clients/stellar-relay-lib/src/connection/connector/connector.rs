use std::fmt::{Debug, Formatter};
use substrate_stellar_sdk::{
	types::{AuthenticatedMessageV0, Curve25519Public, HmacSha256Mac, MessageType},
	XdrCodec,
};
use tokio::net::tcp::OwnedWriteHalf;

use crate::{
	connection::{
		authentication::{gen_shared_key, ConnectionAuth},
		flow_controller::FlowController,
		handshake::HandshakeState,
		hmac::{verify_hmac, HMacKeys},
		ConnectionInfo, Error,
	},
	node::{LocalInfo, NodeInfo, RemoteInfo},
};

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
	pub(crate) write_stream_overlay: OwnedWriteHalf,
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
		write_stream_overlay: OwnedWriteHalf,
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
			write_stream_overlay,
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
	use crate::{connection::hmac::HMacKeys, node::RemoteInfo, StellarOverlayConfig};
	use serial_test::serial;

	use substrate_stellar_sdk::{
		compound_types::LimitedString,
		types::{Hello, MessageType},
		PublicKey,
	};
	use tokio::{io::AsyncWriteExt, net::tcp::OwnedReadHalf};

	use crate::{
		connection::{
			authentication::{create_auth_cert, ConnectionAuth},
			Connector,
		},
		helper::{create_stream, time_now},
		node::NodeInfo,
		ConnectionInfo,
	};

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

	impl Connector {
		fn shutdown(&mut self, read_half: OwnedReadHalf) {
			let _ = self.write_stream_overlay.shutdown();

			drop(read_half);
		}
	}

	async fn create_connector() -> (NodeInfo, ConnectionInfo, Connector, OwnedReadHalf) {
		let cfg_file_path = "./resources/config/testnet/stellar_relay_config_sdftest1.json";
		let secret_key_path = "./resources/secretkey/stellar_secretkey_testnet";
		let secret_key =
			std::fs::read_to_string(secret_key_path).expect("should be able to read file");

		let cfg =
			StellarOverlayConfig::try_from_path(cfg_file_path).expect("should create a config");
		let node_info = cfg.node_info();
		let conn_info = cfg.connection_info(&secret_key).expect("should create a connection info");
		// this is a channel to communicate with the connection/config (this needs renaming)

		let (read_half, write_half) =
			create_stream(&conn_info.address()).await.expect("should return a stream");
		let connector = Connector::new(node_info.clone(), conn_info.clone(), write_half);
		(node_info, conn_info, connector, read_half)
	}

	#[tokio::test]
	#[serial]
	async fn create_new_connector_works() {
		let (node_info, _, mut connector, read_half) = create_connector().await;

		let connector_local_node = connector.local.node();

		assert_eq!(connector_local_node.ledger_version, node_info.ledger_version);
		assert_eq!(connector_local_node.overlay_version, node_info.overlay_version);
		assert_eq!(connector_local_node.overlay_min_version, node_info.overlay_min_version);
		assert_eq!(connector_local_node.version_str, node_info.version_str);
		assert_eq!(connector_local_node.network_id, node_info.network_id);

		connector.shutdown(read_half);
	}

	#[tokio::test]
	#[serial]
	async fn connector_local_sequence_works() {
		let (_, _, mut connector, read_half) = create_connector().await;
		assert_eq!(connector.local_sequence(), 0);
		connector.increment_local_sequence();
		assert_eq!(connector.local_sequence(), 1);

		connector.shutdown(read_half);
	}

	#[tokio::test]
	#[serial]
	async fn connector_set_remote_works() {
		let (_, _, mut connector, read_half) = create_connector().await;

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

		connector.shutdown(read_half);
	}

	#[tokio::test]
	#[serial]
	async fn connector_increment_remote_sequence_works() {
		let (_, _, mut connector, read_half) = create_connector().await;

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

		connector.shutdown(read_half);
	}

	#[tokio::test]
	#[serial]
	async fn connector_get_and_set_hmac_keys_works() {
		//arrange
		let (_, _, mut connector, read_half) = create_connector().await;
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

		connector.shutdown(read_half);
	}

	#[tokio::test]
	#[serial]
	async fn connector_method_works() {
		let (_, conn_config, mut connector, read_half) = create_connector().await;

		assert_eq!(connector.remote_called_us(), conn_config.remote_called_us);
		assert_eq!(connector.receive_tx_messages(), conn_config.recv_tx_msgs);
		assert_eq!(connector.receive_scp_messages(), conn_config.recv_scp_msgs);

		connector.got_hello();
		assert!(connector.is_handshake_created());

		connector.handshake_completed();
		assert!(connector.is_handshake_created());

		connector.shutdown(read_half);
	}

	#[tokio::test]
	#[serial]
	async fn enable_flow_controller_works() {
		let (node_info, _, mut connector, read_half) = create_connector().await;

		assert!(!connector.inner_check_to_send_more(MessageType::ScpMessage));
		connector.enable_flow_controller(node_info.overlay_version, node_info.overlay_version);

		connector.shutdown(read_half);
	}
}
