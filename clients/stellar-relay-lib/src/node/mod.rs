use std::fmt::{Debug, Formatter};

use substrate_stellar_sdk::network::Network;

pub use local::*;
pub use remote::*;

mod local;
mod remote;

pub type NetworkId = [u8; 32];

#[derive(Clone)]
pub struct NodeInfo {
	pub ledger_version: u32,
	pub overlay_version: u32,
	pub overlay_min_version: u32,
	pub version_str: Vec<u8>,
	pub network_id: NetworkId,
}

impl Debug for NodeInfo {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"NodeInfo {{ ledger_version: {}, overlay_version: {}, overlay_min_version: {}, \
			version_str: {} }}",
			self.ledger_version,
			self.overlay_version,
			self.overlay_min_version,
			String::from_utf8_lossy(&self.version_str),
		)
	}
}

impl NodeInfo {
	pub fn new(
		ledger_version: u32,
		overlay_version: u32,
		overlay_min_version: u32,
		version_str: String,
		network: &Network,
	) -> NodeInfo {
		NodeInfo {
			ledger_version,
			overlay_version,
			overlay_min_version,
			version_str: version_str.into_bytes(),
			network_id: *network.get_id(),
		}
	}
}
