mod config;
mod connection;
pub mod node;
#[cfg(test)]
mod tests;

pub(crate) use connection::{
	handshake::{self, HandshakeState},
	ConnectionInfo, Connector, ConnectorActions,
};
pub use connection::{helper, xdr_converter, Error, StellarOverlayConnection, StellarRelayMessage};

pub use substrate_stellar_sdk as sdk;

pub use config::{connect_to_stellar_overlay_network, StellarOverlayConfig};
