
use stellar_relay::sdk::{
    network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
    SecretKey,
};

use stellar_relay::{ConnConfig, node::NodeInfo};

use vault::oracle::{create_handler, prepare_directories};

use tokio::time::Duration;

pub const SAMPLE_VAULT_ADDRESSES_FILTER: &[&str] =
    &["GAP4SFKVFVKENJ7B7VORAYKPB3CJIAJ2LMKDJ22ZFHIAIVYQOR6W3CXF"];

pub const TIER_1_VALIDATOR_IP_TESTNET: &str = "34.235.168.98";
pub const TIER_1_VALIDATOR_IP_PUBLIC: &str = "135.181.16.110";


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
            tracing::info!("envelope size: {:?}", res);
        }

    }
}