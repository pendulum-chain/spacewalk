use crate::node::NodeInfo;
use substrate_stellar_sdk::types::{Curve25519Public, Hello, Uint256};
use substrate_stellar_sdk::PublicKey;

#[derive(Debug, Clone)]
pub struct RemoteInfo {
    sequence: u64,
    pub_key_ecdh: Curve25519Public,
    pub_key: PublicKey,
    nonce: Uint256,
    node: NodeInfo,
}

impl RemoteInfo {
    pub fn new(hello: &Hello) -> Self {
        RemoteInfo {
            sequence: 0,
            pub_key_ecdh: hello.cert.pubkey.clone(),
            pub_key: hello.peer_id.clone(),
            nonce: hello.nonce,
            node: NodeInfo {
                ledger_version: hello.ledger_version,
                overlay_version: hello.overlay_version,
                overlay_min_version: hello.overlay_min_version,
                version_str: hello.version_str.get_vec().clone(),
                network_id: hello.network_id,
            },
        }
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn pub_key_ecdh(&self) -> &Curve25519Public {
        &self.pub_key_ecdh
    }

    pub fn pub_key(&self) -> &PublicKey {
        &self.pub_key
    }

    pub fn nonce(&self) -> Uint256 {
        self.nonce
    }

    pub fn node(&self) -> &NodeInfo {
        &self.node
    }

    pub fn increment_sequence(&mut self) {
        self.sequence += 1;
    }
}
