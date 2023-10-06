use stellar_relay_lib::{
	connect_to_stellar_overlay_network,
	sdk::types::{ScpStatementPledges, StellarMessage},
	ConnectorActions, StellarOverlayConfig, StellarRelayMessage,
};
use substrate_stellar_sdk::{
	types::{MessageType, ScpStatementExternalize},
	Hash, IntoHash, XdrCodec,
};

//arrange
fn get_tx_set_hash(x: &ScpStatementExternalize) -> Hash {
	let scp_value = x.commit.value.get_vec();
	scp_value[0..32].try_into().unwrap()
}

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

	let mut is_sent = false;
	let actions_sender = overlay_connection.get_actions_sender();
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
						ScpStatementPledges::ScpStExternalize(stmt) => {
							if !is_sent {
								let txset_hash = get_tx_set_hash(&stmt);
								println!(
									"Found ScpStExternalize! sending txsethash: {}",
									hex::encode(txset_hash)
								);
								actions_sender
									.send(ConnectorActions::SendMessage(Box::new(
										StellarMessage::GetTxSet(txset_hash),
									)))
									.await
									.expect("should be able to send message");

								is_sent = true;
							}

							"ScpStExternalize"
						},
						ScpStatementPledges::ScpStNominate(_) => "ScpStNominate ",
					};
					log::info!(
						"{} sent StellarMessage of type {} for ledger {}",
						node_id,
						stmt_type,
						slot
					);
				},
				StellarMessage::GeneralizedTxSet(set) => {
					println!("CARLA CARLA CARLA set: {:?}", set.to_base64_encoded_xdr_string());
				},
				StellarMessage::TxSet(set) => {
					let x = set.to_base64_xdr();
					println!(
						"CARLA CARLA CARLA CARLA!!! set txset!!!!!: {:?}",
						std::str::from_utf8(&x)
					);
				},
				_ => {
					println!("the type: {:?}", msg_type)
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
