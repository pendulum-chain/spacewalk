#![allow(dead_code)]

mod collector;
mod constants;
mod errors;
mod handler;
mod storage;
mod types;

#[cfg(test)]
mod tests;

use tokio::time::Duration;

use stellar_relay::sdk::{
    network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
    types::{ScpEnvelope, TransactionSet},
    SecretKey, TransactionEnvelope,
};

use crate::storage::prepare_directories;
use collector::*;
use constants::*;
use errors::Error;
use stellar_relay::{node::NodeInfo, ConnConfig};
use storage::*;
use types::*;

pub use handler::*;


// a filter trait to get only the ones that makes sense to the structure.
pub trait TxHandler<T> {
    fn process_tx(&self, param: &T) -> bool;
}

#[derive(Copy, Clone)]
pub struct FilterTx;


// example:
/*
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	prepare_directories()?;

	let args: Vec<String> = std::env::args().collect();
	let arg_network = &args[1];
	let mut public_network = false;
	let mut tier1_node_ip = TIER_1_VALIDATOR_IP_TESTNET;

	if arg_network == "mainnet" {
		public_network = true;
		tier1_node_ip = TIER_1_VALIDATOR_IP_PUBLIC;
	}
	let network: &Network = if public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

	log::info!(
		"Connected to {:?} through {:?}",
		std::str::from_utf8(network.get_passphrase().as_slice()).unwrap(),
		tier1_node_ip
	);

	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 21, 19, "v19.1.0".to_string(), network);
	let cfg = ConnConfig::new(tier1_node_ip, 11625, secret, 0, true, true, false);

	let vault_addresses_filter =
		vec!["GAP4SFKVFVKENJ7B7VORAYKPB3CJIAJ2LMKDJ22ZFHIAIVYQOR6W3CXF".to_string()];

	let handler = create_handler(node_info, cfg, public_network, vault_addresses_filter).await?;

	loop {
		tokio::time::sleep(Duration::from_secs(8)).await;
		// let's try to send a message?
		let (sender, receiver) = tokio::sync::oneshot::channel();
		handler.get_size(sender).await?;

		if let Ok(res) = receiver.await {
			log::info!("envelope size: {:?}", res);
		}
	}
}


*/