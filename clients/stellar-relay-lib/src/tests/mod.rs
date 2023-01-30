use std::{sync::Arc, time::Duration};
use substrate_stellar_sdk::{
	network::PUBLIC_NETWORK,
	types::{ScpStatementExternalize, ScpStatementPledges, StellarMessage},
	Hash, SecretKey,
};

use crate::{node::NodeInfo, ConnConfig, StellarOverlayConnection, StellarRelayMessage};
use serial_test::serial;
use tokio::{sync::Mutex, time::timeout};

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

	overlay_connection.disconnect().await.expect("Should be able to disconnect");
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
	let mut overlay_connection = Arc::new(Mutex::new(
		StellarOverlayConnection::connect(node_info.clone(), cfg).await.unwrap(),
	));
	let mut ov_conn = overlay_connection.clone();

	let mut scps_vec = Arc::new(Mutex::new(vec![]));
	let mut scps_vec_clone = scps_vec.clone();

	timeout(Duration::from_secs(300), async move {
		let mut ov_conn_locked = ov_conn.lock().await;
		while let Some(relay_message) = ov_conn_locked.listen().await {
			match relay_message {
				StellarRelayMessage::Data { p_id: _, msg_type: _, msg } => match *msg {
					StellarMessage::ScpMessage(msg) => {
						scps_vec_clone.lock().await.push(msg);
						ov_conn_locked.disconnect().await.expect("failed to disconnect");
						break
					},
					_ => {},
				},
				_ => {},
			}
		}
	})
	.await
	.expect("time has elapsed");

	overlay_connection
		.lock()
		.await
		.disconnect()
		.await
		.expect("Should be able to disconnect");
	//assert
	//ensure that we receive some scp message from stellar node
	assert!(!scps_vec.lock().await.is_empty());
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

	let mut overlay_connection = Arc::new(Mutex::new(
		StellarOverlayConnection::connect(node_info.clone(), cfg).await.unwrap(),
	));

	let mut ov_conn = overlay_connection.clone();
	let tx_set_vec = Arc::new(Mutex::new(vec![]));

	let mut tx_set_vec_clone = tx_set_vec.clone();

	timeout(Duration::from_secs(300), async move {
		let mut ov_conn_locked = ov_conn.lock().await;

		while let Some(relay_message) = ov_conn_locked.listen().await {
			match relay_message {
				StellarRelayMessage::Data { p_id: _, msg_type: _, msg } => match *msg {
					StellarMessage::ScpMessage(msg) =>
						if let ScpStatementPledges::ScpStExternalize(stmt) = &msg.statement.pledges
						{
							let txset_hash = get_tx_set_hash(stmt);
							ov_conn_locked
								.send(StellarMessage::GetTxSet(txset_hash))
								.await
								.unwrap();
						},
					StellarMessage::TxSet(set) => {
						tx_set_vec_clone.lock().await.push(set);
						ov_conn_locked.disconnect().await.expect("failed to disconnect");
						break
					},
					_ => {},
				},
				_ => {},
			}
		}
	})
	.await
	.expect("time has elapsed");

	overlay_connection
		.lock()
		.await
		.disconnect()
		.await
		.expect("Should be able to disconnect");
	//arrange
	//ensure that we receive some tx set from stellar node
	assert!(!tx_set_vec.lock().await.is_empty());
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
