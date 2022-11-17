use substrate_stellar_sdk::{network::PUBLIC_NETWORK, types::StellarMessage, SecretKey};

use crate::{node::NodeInfo, ConnConfig, StellarOverlayConnection, StellarRelayMessage};

const TIER_1_VALIDATOR_IP_PUBLIC: &str = "51.161.197.48";
#[tokio::test]
async fn name() {
	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), &PUBLIC_NETWORK);
	let cfg = ConnConfig::new(TIER_1_VALIDATOR_IP_PUBLIC, 11625, secret, 0, false, false, false);
	let mut overlay_conn = StellarOverlayConnection::connect(node_info.clone(), cfg).await.unwrap();

	let message = overlay_conn.listen().await.unwrap();
	if let StellarRelayMessage::Connect{pub_key : x, node_info : y} = message{
		assert_eq!(y.ledger_version, node_info.ledger_version);
	}
	else{
		panic!("Incorrect stellar relay message received");
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
