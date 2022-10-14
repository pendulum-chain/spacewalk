use stellar_relay::{ConnConfig, StellarRelayMessage, StellarOverlayConnection};
use stellar_relay::node::NodeInfo;
use stellar_relay::sdk::{SecretKey, XdrCodec};
use stellar_relay::sdk::network::{Network, PUBLIC_NETWORK, TEST_NETWORK};
use stellar_relay::sdk::types::{MessageType, ScpStatementPledges, StellarMessage};

const TIER_1_VALIDATOR_IP_TESTNET: &str = "34.235.168.98";
const TIER_1_VALIDATOR_IP_PUBLIC: &str = "135.181.16.110";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let arg_network = &args[1];
    let mut public_network = false;
    let mut tier1_node_ip = TIER_1_VALIDATOR_IP_TESTNET;

    if arg_network == "mainnet" {
        public_network = true;
        tier1_node_ip = TIER_1_VALIDATOR_IP_PUBLIC;
    }
    let network: &Network = if public_network {
        &PUBLIC_NETWORK
    } else {
        &TEST_NETWORK
    };

    log::info!(
        "Connected to {:?} through {:?}",
        std::str::from_utf8(network.get_passphrase().as_slice()).unwrap(),
        tier1_node_ip
    );

    let secret =
        SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
            .unwrap();

    let node_info = NodeInfo::new(19, 21, 19, "v19.1.0".to_string(), network);
    let cfg = ConnConfig::new(tier1_node_ip, 11625, secret, 0, false, true, false);
    let mut overlay_connection = StellarOverlayConnection::connect(node_info, cfg).await?;

    while let Some(relay_message) = overlay_connection.listen().await {
        match relay_message {
            StellarRelayMessage::Connect { pub_key, node_info } => {
                let pub_key_xdr = pub_key.to_xdr();
                log::info!("Connected to Stellar Node: {:?}", base64::encode(pub_key_xdr));
                log::info!("{:?}",node_info);
            }
            StellarRelayMessage::Data { p_id, msg_type, msg } => {
                match msg {
                    StellarMessage::ScpMessage(msg) => {
                        let node_id = msg.statement.node_id.to_encoding();
                        let node_id = base64::encode(&node_id);
                        let slot = msg.statement.slot_index;

                        let stmt_type = match msg.statement.pledges {
                            ScpStatementPledges::ScpStPrepare(_) => { "ScpStPrepare" }
                            ScpStatementPledges::ScpStConfirm(_) => { "ScpStConfirm" }
                            ScpStatementPledges::ScpStExternalize(_) => { "ScpStExternalize" }
                            ScpStatementPledges::ScpStNominate(_) => { "ScpStNominate " }
                        };
                        log::info!("{} sent StellarMessage of type {} for ledger {}",
                            node_id,stmt_type,slot
                        );
                    }
                    _ => {
                        log::info!("rcv StellarMessage of type: {:?}", msg_type);
                    }
                }
            }
            StellarRelayMessage::Error(e) => {
                log::error!("Error: {:?}",e);
            }
            StellarRelayMessage::Timeout => {
                log::error!("timed out");
            }
        }
    }

    Ok(())
}