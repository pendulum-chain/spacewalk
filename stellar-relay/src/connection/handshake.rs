use substrate_stellar_sdk::compound_types::LimitedString;

use crate::node::NodeInfo;
use crate::Error;
use substrate_stellar_sdk::types::{Auth, AuthCert, Hello, StellarMessage, Uint256};
use substrate_stellar_sdk::PublicKey;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandshakeState {
    Connecting,
    GotHello,
    Completed,
}

pub fn create_auth_message() -> StellarMessage {
    let auth = Auth { flags: 1 };

    StellarMessage::Auth(auth)
}

pub fn create_hello_message(
    peer_id: PublicKey,
    nonce: Uint256,
    cert: AuthCert,
    listening_port: u32,
    node_info: &NodeInfo,
) -> Result<StellarMessage, Error> {
    let version_str = &node_info.version_str;

    let hello = Hello {
        ledger_version: node_info.ledger_version,
        overlay_version: node_info.overlay_version,
        overlay_min_version: node_info.overlay_min_version,
        network_id: node_info.network_id,
        version_str: LimitedString::<100>::new(version_str.clone())?,
        listening_port: i32::try_from(listening_port).unwrap_or(11625),
        peer_id,
        cert,
        nonce,
    };

    Ok(StellarMessage::Hello(hello))
}
