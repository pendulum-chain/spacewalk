use substrate_stellar_sdk::{
	network::PUBLIC_NETWORK,
	types::{ScpStatementExternalize, ScpStatementPledges, StellarMessage},
	Hash, SecretKey,
};

use crate::{node::NodeInfo, ConnConfig, StellarOverlayConnection, StellarRelayMessage};
use serial_test::serial;

const TIER_1_VALIDATOR_IP_PUBLIC: &str = "51.161.197.48";

#[tokio::test]
#[serial]
async fn stellar_overlay_connect_and_listen_connect_message() {
	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), &PUBLIC_NETWORK);
	let cfg = ConnConfig::new(TIER_1_VALIDATOR_IP_PUBLIC, 11625, secret, 0, false, true, false);
	let mut overlay_connection =
		StellarOverlayConnection::connect(node_info.clone(), cfg).await.unwrap();

	let message = overlay_connection.listen().await.unwrap();
	if let StellarRelayMessage::Connect { pub_key: _x, node_info: y } = message {
		assert_eq!(y.ledger_version, node_info.ledger_version);
	} else {
		panic!("Incorrect stellar relay message received");
	}
}

#[tokio::test]
#[serial]
async fn stellar_overlay_should_receive_scp_messages() {
	//arrange
	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), &PUBLIC_NETWORK);
	//act
	let cfg = ConnConfig::new(TIER_1_VALIDATOR_IP_PUBLIC, 11625, secret, 0, false, true, false);
	let mut overlay_connection =
		StellarOverlayConnection::connect(node_info.clone(), cfg).await.unwrap();

	let mut scps_vec = vec![];
	let mut attempt = 0;
	while let Some(relay_message) = overlay_connection.listen().await {
		if attempt > 20 {
			break
		}
		attempt += 1;
		match relay_message {
			StellarRelayMessage::Data { p_id: _, msg_type: _, msg } => match msg {
				StellarMessage::ScpMessage(msg) => {
					scps_vec.push(msg);
					break
				},
				_ => {},
			},
			_ => {},
		}
	}
	//assert
	//ensure that we receive some scp message from stellar node
	assert!(!scps_vec.is_empty());
}

#[tokio::test]
#[serial]
async fn stellar_overlay_should_receive_tx_set() {
	//arrange
	pub fn get_tx_set_hash(x: &ScpStatementExternalize) -> Hash {
		let scp_value = x.commit.value.get_vec();
		scp_value[0..32].try_into().unwrap()
	}

	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), &PUBLIC_NETWORK);
	let cfg = ConnConfig::new(TIER_1_VALIDATOR_IP_PUBLIC, 11625, secret, 0, true, true, false);
	//act
	let mut overlay_connection =
		StellarOverlayConnection::connect(node_info.clone(), cfg).await.unwrap();

	let mut tx_set_vec = vec![];
	let mut attempt = 0;
	while let Some(relay_message) = overlay_connection.listen().await {
		if attempt > 300 {
			break
		}
		attempt += 1;
		match relay_message {
			StellarRelayMessage::Data { p_id: _, msg_type: _, msg } => match msg {
				StellarMessage::ScpMessage(msg) => {
					if let ScpStatementPledges::ScpStExternalize(stmt) = &msg.statement.pledges {
						let txset_hash = get_tx_set_hash(stmt);
						overlay_connection
							.send(StellarMessage::GetTxSet(txset_hash))
							.await
							.unwrap();
					}
				},
				StellarMessage::TxSet(set) => {
					tx_set_vec.push(set);
					break
				},
				_ => {},
			},
			_ => {},
		}
	}
	//arrange
	//ensure that we receive some tx set from stellar node
	assert!(!tx_set_vec.is_empty());
}

#[tokio::test]
#[serial]
async fn stellar_overlay_disconnect_works() {
	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), &PUBLIC_NETWORK);
	let cfg = ConnConfig::new(TIER_1_VALIDATOR_IP_PUBLIC, 11625, secret, 0, false, false, false);
	let mut overlay_connection =
		StellarOverlayConnection::connect(node_info.clone(), cfg).await.unwrap();

	let message = overlay_connection.listen().await.unwrap();
	if let StellarRelayMessage::Connect { pub_key: _x, node_info: y } = message {
		assert_eq!(y.ledger_version, node_info.ledger_version);
	} else {
		panic!("Incorrect stellar relay message received");
	}
	overlay_connection.disconnect().await.unwrap();
}
