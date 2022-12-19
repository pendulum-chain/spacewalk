use tokio::time::Duration;

use stellar_relay_lib::{
	node::NodeInfo,
	sdk::{
		network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
		SecretKey, TransactionEnvelope,
	},
	ConnConfig,
};
use vault::oracle::{prepare_directories, OracleAgent, Proof, ProofExt, ProofStatus};

pub const SAMPLE_VAULT_ADDRESSES_FILTER: &[&str] =
	&["GAP4SFKVFVKENJ7B7VORAYKPB3CJIAJ2LMKDJ22ZFHIAIVYQOR6W3CXF"];

pub const TIER_1_VALIDATOR_IP_TESTNET: &str = "34.235.168.98";
pub const TIER_1_VALIDATOR_IP_PUBLIC: &str = "65.108.1.53";

// TODO: THIS WILL NOT WORK PROPERLY JUST YET, SINCE IT NEEDS SOME STUFF FROM OTHER PRS.
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

	// let vault_addresses_filter =
	// 	vec!["GAP4SFKVFVKENJ7B7VORAYKPB3CJIAJ2LMKDJ22ZFHIAIVYQOR6W3CXF".to_string()];

	let mut handler = OracleAgent::new(true).expect("should return agent");

	handler.start().await?;

	// this is to test out that
	// 1. "retrieving envelopes from Archives" works; ✓
	// 2. "retrieving envelopes from Stellar Node" works; ✓
	// 3. todo: "retrieving transaction set" works;
	// 4.

	loop {
		tokio::time::sleep(Duration::from_secs(20)).await;
		let slot = handler.get_last_slot_index();

		let res = handler.get_proof(slot).await;
		println!("result: {:?}", res);
	}

	Ok(())
}
