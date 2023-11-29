use crate::{connection::helper::generate_random_nonce, node::NodeInfo};
use substrate_stellar_sdk::types::Uint256;


#[derive(Debug, Clone)]
pub struct LocalInfo {
	sequence: u64,
	nonce: Uint256,
	node: NodeInfo,
	port: u32,
}

impl LocalInfo {
	pub fn new(node: NodeInfo) -> Self {
		LocalInfo { sequence: 0, nonce: generate_random_nonce(), node, port: 11625 }
	}

	pub fn sequence(&self) -> u64 {
		self.sequence
	}

	pub fn nonce(&self) -> Uint256 {
		self.nonce
	}

	pub fn node(&self) -> &NodeInfo {
		&self.node
	}

	pub fn port(&self) -> u32 {
		self.port
	}

	pub fn increment_sequence(&mut self) {
		self.sequence += 1;
	}
}
