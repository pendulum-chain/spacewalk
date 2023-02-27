use std::fmt::Debug;
use substrate_stellar_sdk::network::{Network, PUBLIC_NETWORK, TEST_NETWORK};

use crate::config::NodeInfoCfg;
pub use local::*;
pub use remote::*;

mod local;
mod remote;

pub type NetworkId = [u8; 32];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeInfo {
	pub ledger_version: u32,
	pub overlay_version: u32,
	pub overlay_min_version: u32,
	pub version_str: Vec<u8>,
	pub network_id: NetworkId,
}

impl NodeInfo {
	#[cfg(test)]
	pub(crate) fn new(
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
