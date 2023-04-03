use std::{sync::Arc, time::Duration};
use substrate_stellar_sdk::{
	types::{ScpStatementExternalize, ScpStatementPledges, StellarMessage},
	Hash,
};

use crate::{
	node::NodeInfo, ConnectionInfo, StellarOverlayConfig, StellarOverlayConnection,
	StellarRelayMessage,
};
use serial_test::serial;
use tokio::{sync::Mutex, time::timeout};

fn secret_key() -> String {
	std::fs::read_to_string("./resources/secretkey/stellar_secretkey_testnet")
		.expect("should be able to read file")
}

fn overlay_infos() -> (NodeInfo, ConnectionInfo) {
	let cfg = StellarOverlayConfig::try_from_path(
		"./resources/config/testnet/stellar_relay_config_sdftest1.json",
	)
	.expect("should be able to extract config");

	(cfg.node_info(), cfg.connection_info(&secret_key()).expect("should return conn info"))
}

#[tokio::test]
#[serial]
async fn stellar_overlay_connect_and_listen_connect_message() {
	let (node_info, conn_info) = overlay_infos();

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
	let (node_info, conn_info) = overlay_infos();

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

	let (node_info, conn_info) = overlay_infos();
	let overlay_connection = Arc::new(Mutex::new(
		StellarOverlayConnection::connect(node_info, conn_info).await.unwrap(),
	));

	let ov_conn = overlay_connection.clone();
	let tx_set_vec = Arc::new(Mutex::new(vec![]));
	let tx_set_vec_clone = tx_set_vec.clone();

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

	//arrange
	//ensure that we receive some tx set from stellar node
	assert!(!tx_set_vec.lock().await.is_empty());
}

#[tokio::test]
#[serial]
async fn stellar_overlay_disconnect_works() {
	let (node_info, conn_info) = overlay_infos();

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
