mod errors;
mod flow_controller;
pub(crate) mod handshake;
mod hmac;

mod authentication;
mod connector;
pub mod helper;
mod services;
mod user_controls;
pub mod xdr_converter;

pub(crate) use connector::*;
pub use errors::Error;
pub use services::*;
pub use user_controls::*;

type Xdr = (u32, Vec<u8>);

use crate::node::NodeInfo;
use substrate_stellar_sdk::types::{MessageType, StellarMessage};
use substrate_stellar_sdk::{PublicKey, SecretKey};

#[derive(Debug)]
/// Represents the messages that the connection creates bases on the Stellar Node
pub enum StellarNodeMessage {
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
    Timeout,
}

/// Config for connecting to Stellar Node
pub struct ConnConfig {
    address: String,
    port: u32,
    secret_key: SecretKey,
    pub auth_cert_expiration: u64,
    pub recv_tx_msgs: bool,
    pub recv_scp_messages: bool,
    pub remote_called_us: bool,
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
    ) -> ConnConfig {
        ConnConfig {
            address: addr.to_owned(),
            port,
            secret_key,
            auth_cert_expiration,
            recv_tx_msgs,
            recv_scp_messages,
            remote_called_us,
        }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }

    pub fn keypair(&self) -> SecretKey {
        self.secret_key.clone()
    }
}
