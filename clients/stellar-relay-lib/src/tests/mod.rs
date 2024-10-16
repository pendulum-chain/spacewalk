use crate::{
	connection::ConnectionInfo, node::NodeInfo, StellarOverlayConfig, StellarOverlayConnection,
};
use async_std::sync::Mutex;
use serial_test::serial;
use std::{sync::Arc, thread::sleep, time::Duration};
use substrate_stellar_sdk::{
	types::{ScpStatementExternalize, ScpStatementPledges, StellarMessage},
	Hash, IntoHash,
};

use wallet::keys::get_source_secret_key_from_env;

fn secret_key(is_mainnet: bool) -> String {
	get_source_secret_key_from_env(is_mainnet)
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

	(cfg.node_info(), cfg.connection_info(secret_key(is_mainnet)).expect("should return conn info"))
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn stellar_overlay_should_receive_scp_messages() {
	let (node_info, conn_info) = overlay_infos(false);

	let overlay_connection = Arc::new(Mutex::new(
		StellarOverlayConnection::connect(node_info, conn_info)
			.await
			.expect("should connect"),
	));

	// to check if we receive any scp message from stellar node
	let (sender, receiver) = tokio::sync::oneshot::channel();
	let ov_conn = overlay_connection.clone();

	let scps_vec = Arc::new(Mutex::new(vec![]));
	let scps_vec_clone = scps_vec.clone();
	tokio::spawn(async move {
		let mut ov_conn_locked = ov_conn.lock().await;
		loop {
			if let Some(msg) = ov_conn_locked.listen().await.expect("should return a message") {
				scps_vec_clone.lock().await.push(msg);
				sender.send(()).unwrap();
				break;
			}
		}

		ov_conn_locked.stop();
	})
	.await
	.expect("should finish");

	let _ = receiver.await.expect("should receive a message");

	//assert
	//ensure that we receive some scp message from stellar node
	assert!(!scps_vec.lock().await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
#[ntest::timeout(300_000)] // timeout at 5 minutes
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

	let mut ov_conn_locked = ov_conn.lock().await;

	loop {
		if let Ok(Some(msg)) = ov_conn_locked.listen().await {
			match msg {
				StellarMessage::ScpMessage(msg) => {
					if let ScpStatementPledges::ScpStExternalize(stmt) = &msg.statement.pledges {
						let tx_set_hash = get_tx_set_hash(stmt);
						tx_set_hashes_clone.lock().await.push(tx_set_hash.clone());
						ov_conn_locked
							.send_to_node(StellarMessage::GetTxSet(tx_set_hash))
							.await
							.unwrap();
					}
				},
				StellarMessage::TxSet(set) => {
					let tx_set_hash = set.into_hash().expect("should return a hash");
					actual_tx_set_hashes_clone.lock().await.push(tx_set_hash);
					break;
				},
				StellarMessage::GeneralizedTxSet(set) => {
					let tx_set_hash = set.into_hash().expect("should return a hash");
					actual_tx_set_hashes_clone.lock().await.push(tx_set_hash);
					break;
				},
				_ => {},
			}
		}
	}

	//ensure that we receive some tx set from stellar node
	let expected_hashes = tx_set_hashes.lock().await;
	assert!(!expected_hashes.is_empty());

	let actual_hashes = actual_tx_set_hashes.lock().await;
	assert!(!actual_hashes.is_empty());

	assert!(expected_hashes.contains(&actual_hashes[0]))
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
#[ntest::timeout(300_000)] // timeout at 5 minutes
async fn stellar_overlay_disconnect_works() {
	let (node_info, conn_info) = overlay_infos(false);

	let mut overlay_connection =
		StellarOverlayConnection::connect(node_info.clone(), conn_info).await.unwrap();

	loop {
		if let Some(message) = overlay_connection.listen().await.expect("should return a message") {
			match message {
				// fail the test case if an error message is received
				StellarMessage::ErrorMsg(_) => panic!("Error message received: {:?}", message),
				// it means it has received a message from stellar node
				_ => break,
			}
		}
	}

	overlay_connection.stop();

	// let the disconnection call pass for a few seconds, before checking its status.
	sleep(Duration::from_secs(15));
	assert!(!overlay_connection.is_alive());
}
