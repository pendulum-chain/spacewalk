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

type Xdr = (u32, Vec<u8>);

use crate::node::NodeInfo;
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
        msg: StellarMessage,
    },
    Error(String),
    /// The amount of time to wait for Stellar Node messages
    Timeout,
}

/// Config for connecting to Stellar Node
#[derive(Clone)]
pub struct ConnConfig {
    address: String,
    port: u32,
    secret_key: SecretKey,
    pub auth_cert_expiration: u64,
    pub recv_tx_msgs: bool,
    pub recv_scp_messages: bool,
    pub remote_called_us: bool,
    /// how long to wait for the Stellar Node's messages.
    timeout_in_secs: u64,
    /// number of retries to wait for the Stellar Node's messages and/or to connect back to it.
    retries: u8,
}

impl ConnConfig {
    pub fn new(
        addr: &str,
        port: u32,
        secret_key: SecretKey,
        auth_cert_expiration: u64,
        recv_tx_msgs: bool,
        recv_scp_messages: bool,
        remote_called_us: bool,
    ) -> Self {
        Self::new_with_timeout_and_retries(
            addr,
            port,
            secret_key,
            auth_cert_expiration,
            recv_tx_msgs,
            recv_scp_messages,
            remote_called_us,
            10,
            3,
        )
    }

    pub fn new_with_timeout_and_retries(
        addr: &str,
        port: u32,
        secret_key: SecretKey,
        auth_cert_expiration: u64,
        recv_tx_msgs: bool,
        recv_scp_messages: bool,
        remote_called_us: bool,
        timeout_in_secs: u64,
        retries: u8,
    ) -> Self {
        ConnConfig {
            address: addr.to_string(),
            port,
            secret_key,
            auth_cert_expiration,
            recv_tx_msgs,
            recv_scp_messages,
            remote_called_us,
            timeout_in_secs,
            retries,
        }
    }

    pub fn set_timeout_in_secs(&mut self, secs: u64) {
        self.timeout_in_secs = secs;
    }

    pub fn set_retries(&mut self, retries: u8) {
        self.retries = retries;
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }

    pub fn keypair(&self) -> SecretKey {
        self.secret_key.clone()
    }
}
