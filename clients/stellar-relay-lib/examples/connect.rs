use stellar_relay_lib::{
	connect_to_stellar_overlay_network,
	sdk::types::{ScpStatementPledges, StellarMessage},
	StellarOverlayConfig,
};

use wallet::keys::get_source_secret_key_from_env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();

	let args: Vec<String> = std::env::args().collect();
	let arg_network = if args.len() > 1 { &args[1] } else { "testnet" };

	let cfg_file_path = if arg_network == "mainnet" {
		"./clients/stellar-relay-lib/resources/config/mainnet/stellar_relay_config_mainnet_iowa.json"
	} else {
		"./clients/stellar-relay-lib/resources/config/testnet/stellar_relay_config_sdftest1.json"
	};
	let cfg = StellarOverlayConfig::try_from_path(cfg_file_path)?;

	let secret_key = get_source_secret_key_from_env(arg_network == "mainnet");

	let mut overlay_connection = connect_to_stellar_overlay_network(cfg, &secret_key).await?;

	while let Ok(Some(msg)) = overlay_connection.listen() {
		match msg {
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
				tracing::info!(
					"{} sent StellarMessage of type {} for ledger {}",
					node_id,
					stmt_type,
					slot
				);
			},
			_ => {
				let _ = overlay_connection.send_to_node(StellarMessage::GetPeers).await;
			},
		}
	}

	Ok(())
}
