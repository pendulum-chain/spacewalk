use stellar_relay_lib::{
	connect_to_stellar_overlay_network,
	sdk::types::{ScpStatementPledges, StellarMessage},
	StellarOverlayConfig, StellarRelayMessage,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();

	let args: Vec<String> = std::env::args().collect();
	let arg_network = if args.len() > 1 { &args[1] } else { "testnet" };

	let (cfg_file_path, sk_file_path) = if arg_network == "mainnet" {
		(
			"./clients/stellar-relay-lib/resources/config/mainnet/stellar_relay_config_mainnet_iowa.json",
			"./clients/stellar-relay-lib/resources/secretkey/stellar_secretkey_mainnet",
		)
	} else {
		(
			"./clients/stellar-relay-lib/resources/config/testnet/stellar_relay_config_sdftest1.json",
			"./clients/stellar-relay-lib/resources/secretkey/stellar_secretkey_testnet",
		)
	};
	let cfg = StellarOverlayConfig::try_from_path(cfg_file_path)?;
	let secret_key = std::fs::read_to_string(sk_file_path)?;

	let mut overlay_connection = connect_to_stellar_overlay_network(cfg, &secret_key).await?;

	while let Some(relay_message) = overlay_connection.listen().await {
		match relay_message {
			StellarRelayMessage::Connect { pub_key, node_info } => {
				let pub_key = pub_key.to_encoding();
				let pub_key = std::str::from_utf8(&pub_key).expect("should work?");
				log::info!("Connected to Stellar Node: {pub_key}");
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
			StellarRelayMessage::Disconnect => {
				log::error!("timed out");
			},
		}
	}

	Ok(())
}
