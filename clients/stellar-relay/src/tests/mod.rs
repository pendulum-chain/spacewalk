use substrate_stellar_sdk::{network::PUBLIC_NETWORK, types::{StellarMessage, ScpStatementPledges, ScpStatementExternalize}, SecretKey};

use crate::{node::NodeInfo, ConnConfig, StellarOverlayConnection, StellarRelayMessage};

const TIER_1_VALIDATOR_IP_PUBLIC: &str = "51.161.197.48";
#[tokio::test]
async fn stellar_overlay_connect_and_listen_connect_message() {
	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), &PUBLIC_NETWORK);
	let cfg = ConnConfig::new(TIER_1_VALIDATOR_IP_PUBLIC, 11625, secret, 0, false, true, false);
	let mut overlay_connection = StellarOverlayConnection::connect(node_info.clone(), cfg).await.unwrap();

	let message = overlay_connection.listen().await.unwrap();
	if let StellarRelayMessage::Connect{pub_key : x, node_info : y} = message{
		assert_eq!(y.ledger_version, node_info.ledger_version);
	}
	else{
		panic!("Incorrect stellar relay message received");
	}

	let mut attempt = 0;
	while let Some(relay_message) = overlay_connection.listen().await {
		if attempt > 20{
			break;
		}
		// println!("{:#?}", relay_message);
		attempt = attempt + 1;
		match relay_message {
			StellarRelayMessage::Connect { pub_key, node_info } => {
				// let pub_key_xdr = pub_key.to_xdr();
				// log::info!("Connected to Stellar Node: {:?}", base64::encode(pub_key_xdr));
				// log::info!("{:?}", node_info);
			},
			StellarRelayMessage::Data { p_id, msg_type, msg } => match msg {
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
					println!(
						"{} sent StellarMessage of type {} for ledger {}",
						node_id,
						stmt_type,
						slot
					);
				},
				_ => {
					println!("rcv StellarMessage of type: {:?}", msg_type);
				},
			},
			StellarRelayMessage::Error(e) => {
				println!("Error: {:?}", e);
			},
			StellarRelayMessage::Timeout => {
				println!("timed out");
			},
		}
	}
	// overlay_conn.send(StellarMessage::GetScpState(1)).await.unwrap();
	// println!("{:#?}", message);
	// match message {
	// 	StellarRelayMessage::Data { p_id: _, msg_type, msg } => match msg {
	// 		StellarMessage::ScpMessage(env) => {},
	// 		StellarMessage::TxSet(set) => {},
	// 		_ => {},
	// 	},
	// 	StellarRelayMessage::Connect { pub_key: _, node_info: _ } => {},
	// 	StellarRelayMessage::Error(_) => {},
	// 	StellarRelayMessage::Timeout => {},
	// }
}

#[tokio::test]
async fn stellar_overlay_should_receive_scp_messages() {
	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), &PUBLIC_NETWORK);
	let cfg = ConnConfig::new(TIER_1_VALIDATOR_IP_PUBLIC, 11625, secret, 0, false, true, false);
	let mut overlay_connection = StellarOverlayConnection::connect(node_info.clone(), cfg).await.unwrap();

	let mut scps_vec = vec![];
	let mut attempt = 0;
	while let Some(relay_message) = overlay_connection.listen().await {
		if attempt > 20{
			break;
		}
		attempt = attempt + 1;
		match relay_message {
			StellarRelayMessage::Data { p_id, msg_type, msg } => match msg {
				StellarMessage::ScpMessage(msg) => {
					scps_vec.push(msg);
					break;
				},
				_ => {
				},
			},
			_ => {}
		}
	}
	assert!(scps_vec.len() > 0);
}
use substrate_stellar_sdk::Hash;
#[tokio::test]
async fn stellar_overlay_should_receive_tx_set() {

	pub fn get_tx_set_hash(x: &ScpStatementExternalize) -> Hash {
		let scp_value = x.commit.value.get_vec();
		return scp_value[0..32].try_into().unwrap();
	}

	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), &PUBLIC_NETWORK);
	let cfg = ConnConfig::new(TIER_1_VALIDATOR_IP_PUBLIC, 11625, secret, 0, true, true, false);
	let mut overlay_connection = StellarOverlayConnection::connect(node_info.clone(), cfg).await.unwrap();

	let mut scps_vec = vec![];
	let mut attempt = 0;
	while let Some(relay_message) = overlay_connection.listen().await {
		if attempt > 300{
			break;
		}
		attempt = attempt + 1;
		match relay_message {
			StellarRelayMessage::Data { p_id, msg_type, msg } => match msg {
				StellarMessage::ScpMessage(msg) => {
					if let ScpStatementPledges::ScpStExternalize(stmt) = &msg.statement.pledges {
						let txset_hash = get_tx_set_hash(stmt);
						overlay_connection.send(StellarMessage::GetTxSet(txset_hash)).await.unwrap();
					}
				},
				StellarMessage::TxSet(set) => {
					scps_vec.push(set);
					break;
				},
				_ => {
				},
			},
			_ => {}
		}
	}
	assert!(scps_vec.len() > 0);
}
