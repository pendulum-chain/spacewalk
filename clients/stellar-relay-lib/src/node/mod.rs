use std::fmt::{Debug, Formatter};
use substrate_stellar_sdk::network::{Network, PUBLIC_NETWORK, TEST_NETWORK};

use crate::config::NodeInfoCfg;
pub use local::*;
pub use remote::*;

mod local;
mod remote;

pub type NetworkId = [u8; 32];

#[derive(Clone, PartialEq, Eq)]
pub struct NodeInfo {
	pub ledger_version: u32,
	pub overlay_version: u32,
	pub overlay_min_version: u32,
	pub version_str: Vec<u8>,
	pub network_id: NetworkId,
}

impl NodeInfo {
	pub(crate) fn new(cfg:&NodeInfoCfg) -> Self {
		let network: &Network = if cfg.is_pub_net { &PUBLIC_NETWORK } else { &TEST_NETWORK };
		NodeInfo {
			ledger_version: cfg.ledger_version,
			overlay_version: cfg.overlay_version,
			overlay_min_version: cfg.overlay_min_version,
			version_str: cfg.version_str.clone(),
			network_id: *network.get_id(),
		}
	}
}

impl Debug for NodeInfo {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NodeInfo")
			.field("ledger_version", &self.ledger_version)
			.field("overlay_version", &self.overlay_version)
			.field("overlay_min_version", &self.overlay_min_version)
			.field("version_str", &String::from_utf8_lossy(&self.version_str))
			.finish()
	}
}

impl From<NodeInfoCfg> for NodeInfo {
	fn from(value: NodeInfoCfg) -> Self {
		let network: &Network = if value.is_pub_net { &PUBLIC_NETWORK } else { &TEST_NETWORK };

		NodeInfo {
			ledger_version: value.ledger_version,
			overlay_version: value.overlay_version,
			overlay_min_version: value.overlay_min_version,
			version_str: value.version_str,
			network_id: *network.get_id(),
		}
	}
}