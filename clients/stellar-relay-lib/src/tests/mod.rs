use std::{sync::Arc, time::Duration};
use substrate_stellar_sdk::{
	types::{ScpStatementExternalize, ScpStatementPledges, StellarMessage},
	Hash, IntoHash,
};

use crate::{
	node::NodeInfo, ConnectionInfo, StellarOverlayConfig, StellarOverlayConnection,
	StellarRelayMessage,
};
use serial_test::serial;
use tokio::{sync::Mutex, time::timeout};

fn secret_key(is_mainnet: bool) -> String {
	let path = if is_mainnet {
		"./resources/secretkey/stellar_secretkey_mainnet"
	} else {
		"./resources/secretkey/stellar_secretkey_testnet"
	};

	std::fs::read_to_string(path).expect("should be able to read file")
}

fn overlay_infos(is_mainnet: bool) -> (NodeInfo, ConnectionInfo) {
	use rand::seq::SliceRandom;

	let path = if is_mainnet {
		"./resources/config/mainnet/stellar_relay_config_mainnet_iowa.json".to_string()
	} else {
		let stellar_node_points = [1, 2, 3];
		let node_point = stellar_node_points
			.choose(&mut rand::thread_rng())
			.expect("should return a value");

		format!("./resources/config/testnet/stellar_relay_config_sdftest{node_point}.json")
	};

	let cfg = StellarOverlayConfig::try_from_path(&path).expect("should be able to extract config");

	(
		cfg.node_info(),
		cfg.connection_info(&secret_key(is_mainnet)).expect("should return conn info"),
	)
}

#[tokio::test]
#[serial]
async fn stellar_overlay_connect_and_listen_connect_message() {
	let (node_info, conn_info) = overlay_infos(false);

	let mut overlay_connection =
		StellarOverlayConnection::connect(node_info.clone(), conn_info).await.unwrap();

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
	let (node_info, conn_info) = overlay_infos(false);

	let overlay_connection = Arc::new(Mutex::new(
		StellarOverlayConnection::connect(node_info, conn_info).await.unwrap(),
	));
	let ov_conn = overlay_connection.clone();

	let scps_vec = Arc::new(Mutex::new(vec![]));
	let scps_vec_clone = scps_vec.clone();

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

	//assert
	//ensure that we receive some scp message from stellar node
	assert!(!scps_vec.lock().await.is_empty());
}

#[tokio::test]
#[serial]
async fn stellar_overlay_should_receive_tx_set() {
	//arrange
	fn get_tx_set_hash(x: &ScpStatementExternalize) -> Hash {
		let scp_value = x.commit.value.get_vec();
		scp_value[0..32].try_into().unwrap()
	}

	let (node_info, conn_info) = overlay_infos(false);
	let overlay_connection = Arc::new(Mutex::new(
		StellarOverlayConnection::connect(node_info, conn_info).await.unwrap(),
	));

	let ov_conn = overlay_connection.clone();
	let tx_set_hashes = Arc::new(Mutex::new(vec![]));
	let actual_tx_set_hashes = Arc::new(Mutex::new(vec![]));
	let tx_set_hashes_clone = tx_set_hashes.clone();
	let actual_tx_set_hashes_clone = actual_tx_set_hashes.clone();

	timeout(Duration::from_secs(500), async move {
		let mut ov_conn_locked = ov_conn.lock().await;

		while let Some(relay_message) = ov_conn_locked.listen().await {
			match relay_message {
				StellarRelayMessage::Data { p_id: _, msg_type: _, msg } => match *msg {
					StellarMessage::ScpMessage(msg) =>
						if let ScpStatementPledges::ScpStExternalize(stmt) = &msg.statement.pledges
						{
							let tx_set_hash = get_tx_set_hash(stmt);
							tx_set_hashes_clone.lock().await.push(tx_set_hash.clone());
							ov_conn_locked
								.send(StellarMessage::GetTxSet(tx_set_hash))
								.await
								.unwrap();
						},
					StellarMessage::TxSet(set) => {
						let tx_set_hash = set.into_hash().expect("should return a hash");
						actual_tx_set_hashes_clone.lock().await.push(tx_set_hash);

						ov_conn_locked.disconnect().await.expect("failed to disconnect");
						break
					},
					StellarMessage::GeneralizedTxSet(set) => {
						let tx_set_hash = set.into_hash().expect("should return a hash");
						actual_tx_set_hashes_clone.lock().await.push(tx_set_hash);

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

	//ensure that we receive some tx set from stellar node
	let expected_hashes = tx_set_hashes.lock().await;
	assert!(!expected_hashes.is_empty());

	let actual_hashes = actual_tx_set_hashes.lock().await;
	assert!(!actual_hashes.is_empty());

	assert!(expected_hashes.contains(&actual_hashes[0]))
}

#[tokio::test]
#[serial]
async fn stellar_overlay_disconnect_works() {
	let (node_info, conn_info) = overlay_infos(false);

	let mut overlay_connection =
		StellarOverlayConnection::connect(node_info.clone(), conn_info).await.unwrap();

	let message = overlay_connection.listen().await.unwrap();
	if let StellarRelayMessage::Connect { pub_key: _x, node_info: y } = message {
		assert_eq!(y.ledger_version, node_info.ledger_version);
	} else {
		panic!("Incorrect stellar relay message received");
	}
	overlay_connection.disconnect().await.unwrap();
}
