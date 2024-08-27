use substrate_stellar_sdk::compound_types::LimitedString;

use crate::{connection::Error, node::NodeInfo};
use substrate_stellar_sdk::{
	types::{Auth, AuthCert, Hello, StellarMessage, Uint256},
	PublicKey,
};

// We enable flowControlWithBytes conditionally. This should be enabled if the node version is
// higher than 28. https://github.com/stellarbeat/js-stellar-node-connector/blob/9120f24b75867700ba0ece76369166959205c6b9/src/connection/connection.ts#L634
// Will not be needed anymore after the minimum version is set to 28.
pub const AUTH_FLAG_BYTES_CONTROL: i32 = 200;
pub const AUTH_FLAG_CONTROL: i32 = 100;
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandshakeState {
	Connecting,
	GotHello,
	Completed,
}

pub fn create_auth_message(local_overlay_version: u32) -> StellarMessage {
	let flags =
		if local_overlay_version >= 28 { AUTH_FLAG_BYTES_CONTROL } else { AUTH_FLAG_CONTROL };
	let auth = Auth { flags };

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
