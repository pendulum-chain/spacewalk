use stellar_relay::sdk::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	SecretKey, TransactionEnvelope,
};

use stellar_relay::{node::NodeInfo, ConnConfig};

use vault::oracle::{create_handler, prepare_directories, FilterWith, ProofStatus};

use tokio::time::Duration;

pub const SAMPLE_VAULT_ADDRESSES_FILTER: &[&str] =
	&["GAP4SFKVFVKENJ7B7VORAYKPB3CJIAJ2LMKDJ22ZFHIAIVYQOR6W3CXF"];

pub const TIER_1_VALIDATOR_IP_TESTNET: &str = "34.235.168.98";
pub const TIER_1_VALIDATOR_IP_PUBLIC: &str = "65.108.1.53";

pub struct NoFilter;

// Dummy filter that does nothing.
impl FilterWith<TransactionEnvelope> for NoFilter {
	fn name(&self) -> &'static str {
		"NoFilter"
	}

	fn check_for_processing(&self, _param: &TransactionEnvelope) -> bool {
		false
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	prepare_directories()?;

	let args: Vec<String> = std::env::args().collect();
	let arg_network = if args.len() > 1 { &args[1] } else { "testnet" };
	let mut public_network = false;
	let mut tier1_node_ip = TIER_1_VALIDATOR_IP_TESTNET;

	if arg_network == "mainnet" {
		public_network = true;
		tier1_node_ip = TIER_1_VALIDATOR_IP_PUBLIC;
	}
	let network: &Network = if public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

	tracing::info!(
		"Connected to {:?} through {:?}",
		std::str::from_utf8(network.get_passphrase().as_slice()).unwrap(),
		tier1_node_ip
	);

	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), network);
	let cfg = ConnConfig::new(tier1_node_ip, 11625, secret, 0, true, true, false);

	let vault_addresses_filter =
		vec!["GAP4SFKVFVKENJ7B7VORAYKPB3CJIAJ2LMKDJ22ZFHIAIVYQOR6W3CXF".to_string()];

	let handler = create_handler(node_info, cfg, public_network, vault_addresses_filter).await?;

	let mut counter = 0;
	loop {
		counter += 1;
		tokio::time::sleep(Duration::from_secs(8)).await;
		// let's try to send a message?
		let last_slot = handler.get_last_slot_index().await?;
		tracing::info!("last slot: {:?}", last_slot);

		if counter % 5 == 0 {
			let check_slot = last_slot - 100;
			let res = handler.get_proof(check_slot).await?;
			tracing::info!("proof of slot {}:\n {:?}", check_slot, res);
		} else if counter % 10 == 0 {
			handler.watch_slot(last_slot + 10).await?;
		}
	}
}
