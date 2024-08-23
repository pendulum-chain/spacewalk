use substrate_stellar_sdk::compound_types::LimitedString;

use crate::{connection::Error, node::NodeInfo};
use substrate_stellar_sdk::{
	types::{Auth, AuthCert, Hello, StellarMessage, Uint256},
	PublicKey,
};

// We enable by default flowControlWithBytes
// https://github.com/stellarbeat/js-stellar-node-connector/blob/9120f24b75867700ba0ece76369166959205c6b9/src/connection/flow-controller.ts#L39
pub const AUTH_FLAG: i32 = 200;
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandshakeState {
	Connecting,
	GotHello,
	Completed,
}

pub fn create_auth_message() -> StellarMessage {
	// 200 for flowControlInBytes enabled.
	let auth = Auth { flags: AUTH_FLAG };

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
		version_str: LimitedString::<100>::new(version_str.clone()).map_err(|e| {
			tracing::error!("create_hello_message(): {e:?}");
			Error::VersionStrTooLong
		})?,
		listening_port: i32::try_from(listening_port).unwrap_or(11625),
		peer_id,
		cert,
		nonce,
	};

	Ok(StellarMessage::Hello(hello))
}
