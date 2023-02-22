use stellar_relay_lib::{
	connect,
	sdk::{
		types::{ScpStatementPledges, StellarMessage},
		XdrCodec,
	},
	StellarOverlayConfig, StellarOverlayConnection, StellarRelayMessage,
};

const TIER_1_VALIDATOR_IP_TESTNET: &str = "34.235.168.98";
const TIER_1_VALIDATOR_IP_PUBLIC: &str = "51.161.197.48";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();

	let args: Vec<String> = std::env::args().collect();
	let arg_network = if args.len() > 1 { &args[1] } else { "testnet" };
	let mut public_network = false;
	let mut tier1_node_ip = TIER_1_VALIDATOR_IP_TESTNET;

	let file_path = if arg_network == "mainnet" {
		"./clients/stellar-relay-lib/resources/stellar_relay_config_mainnet_iowa.json"
	} else {
		"./clients/stellar-relay-lib/resources/stellar_relay_config_testnet.json"
	};

	let cfg = StellarOverlayConfig::try_from_path(file_path)?;

	let mut overlay_connection = connect(cfg).await?;

	while let Some(relay_message) = overlay_connection.listen().await {
		match relay_message {
			StellarRelayMessage::Connect { pub_key, node_info } => {
				let pub_key_xdr = pub_key.to_xdr();
				log::info!("Connected to Stellar Node: {:?}", base64::encode(pub_key_xdr));
				log::info!("{:?}", node_info);
			},
			StellarRelayMessage::Data { p_id: _, msg_type, msg } => match *msg {
				StellarMessage::ScpMessage(msg) => {
					let node_id = msg.statement.node_id.to_encoding();
					let node_id = base64::encode(&node_id);
					let slot = msg.statement.slot_index;

					let stmt_type = match msg.statement.pledges {
						ScpStatementPledges::ScpStPrepare(_) => "ScpStPrepare",
						ScpStatementPledges::ScpStConfirm(_) => "ScpStConfirm",
						ScpStatementPledges::ScpStExternalize(_) => "ScpStExternalize",
						ScpStatementPledges::ScpStNominate(_) => "ScpStNominate ",
					};
					log::info!(
						"{} sent StellarMessage of type {} for ledger {}",
						node_id,
						stmt_type,
						slot
					);
				},
				_ => {
					log::info!("rcv StellarMessage of type: {:?}", msg_type);
				},
			},
			StellarRelayMessage::Error(e) => {
				log::error!("Error: {:?}", e);
			},
			StellarRelayMessage::Timeout => {
				log::error!("timed out");
			},
		}
	}

	Ok(())
}
