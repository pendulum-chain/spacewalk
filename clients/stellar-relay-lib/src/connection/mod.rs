mod errors;
mod flow_controller;
pub(crate) mod handshake;
mod hmac;

mod authentication;
mod connector;
pub mod helper;
mod overlay_connection;
mod services;
pub mod xdr_converter;

pub(crate) use connector::*;
pub use errors::Error;
pub use overlay_connection::*;
use serde::Serialize;
use std::fmt::{Debug, Formatter};

type Xdr = (u32, Vec<u8>);

use crate::{config::ConnectionInfoCfg, node::NodeInfo};
use substrate_stellar_sdk::{
	types::{MessageType, StellarMessage},
	PublicKey, SecretKey,
};

#[derive(Debug)]
/// Represents the messages that the connection creates bases on the Stellar Node
pub enum StellarRelayMessage {
	/// Successfully connected to the node
	Connect {
		pub_key: PublicKey,
		node_info: NodeInfo,
	},
	/// Stellar messages from the node
	Data {
		p_id: u32,
		msg_type: MessageType,
		msg: Box<StellarMessage>,
	},
	Error(String),
	/// The amount of time to wait for Stellar Node messages
	Timeout,
}

/// Config for connecting to Stellar Node
#[derive(Clone, Serialize, PartialEq, Eq)]
pub struct ConnectionInfo {
	address: String,
	port: u32,
	#[serde(skip_serializing)]
	secret_key: SecretKey,
	pub auth_cert_expiration: u64,
	pub recv_tx_msgs: bool,
	pub recv_scp_msgs: bool,
	pub remote_called_us: bool,
	/// how long to wait for the Stellar Node's messages.
	timeout_in_secs: u64,
	/// number of retries to wait for the Stellar Node's messages and/or to connect back to it.
	retries: u8,
}

impl Debug for ConnectionInfo {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ConnectionInfo")
			.field("address", &self.address)
			.field("port", &self.port)
			// do not expose the secret key
			.field("secret_key", &"****")
			.field("auth_cert_expiration", &self.auth_cert_expiration)
			.field("receive_tx_messages", &self.recv_tx_msgs)
			.field("receive_scp_messages", &self.recv_scp_msgs)
			.field("remote_called_us", &self.remote_called_us)
			.field("timeout_in_seconds", &self.timeout_in_secs)
			.field("retries", &self.retries)
			.finish()
	}
}

impl TryFrom<ConnectionInfoCfg> for ConnectionInfo {
	type Error = Error;

	fn try_from(cfg: ConnectionInfoCfg) -> Result<Self, Self::Error> {
		let secret_key = std::str::from_utf8(cfg.secret_key())
			.map_err(|e| Error::ConfigError(format!("Secret Key: {:?}", e)))?;
		let secret_key = SecretKey::from_encoding(secret_key)?;

		let address = std::str::from_utf8(&cfg.address)
			.map_err(|e| Error::ConfigError(format!("Address: {:?}", e)))?;

		Ok(ConnectionInfo::new_with_timeout_and_retries(
			address,
			cfg.port,
			secret_key,
			cfg.auth_cert_expiration,
			cfg.recv_tx_msgs,
			cfg.recv_scp_msgs,
			cfg.remote_called_us,
			cfg.timeout_in_secs,
			cfg.retries,
		))
	}
}

impl ConnectionInfo {
	#[allow(clippy::too_many_arguments)]
	fn new_with_timeout_and_retries(
		addr: &str,
		port: u32,
		secret_key: SecretKey,
		auth_cert_expiration: u64,
		recv_tx_msgs: bool,
		recv_scp_msgs: bool,
		remote_called_us: bool,
		timeout_in_secs: u64,
		retries: u8,
	) -> Self {
		ConnectionInfo {
			address: addr.to_string(),
			port,
			secret_key,
			auth_cert_expiration,
			recv_tx_msgs,
			recv_scp_msgs,
			remote_called_us,
			timeout_in_secs,
			retries,
		}
	}

	#[cfg(test)]
	pub(crate) fn new(
		addr: &str,
		port: u32,
		secret_key: SecretKey,
		auth_cert_expiration: u64,
		recv_tx_msgs: bool,
		recv_scp_msgs: bool,
		remote_called_us: bool,
	) -> Self {
		Self::new_with_timeout_and_retries(
			addr,
			port,
			secret_key,
			auth_cert_expiration,
			recv_tx_msgs,
			recv_scp_msgs,
			remote_called_us,
			10,
			3,
		)
	}

	pub fn address(&self) -> String {
		format!("{}:{}", self.address, self.port)
	}

	pub fn keypair(&self) -> SecretKey {
		self.secret_key.clone()
	}
}
