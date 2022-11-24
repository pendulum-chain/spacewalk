use stellar_relay_lib::sdk::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	SecretKey, TransactionEnvelope,
};

use stellar_relay_lib::{node::NodeInfo, ConnConfig};

use vault::oracle::{create_handler, prepare_directories, FilterWith, Proof, ProofStatus};

use tokio::time::Duration;

pub const SAMPLE_VAULT_ADDRESSES_FILTER: &[&str] =
	&["GAP4SFKVFVKENJ7B7VORAYKPB3CJIAJ2LMKDJ22ZFHIAIVYQOR6W3CXF"];

pub const TIER_1_VALIDATOR_IP_TESTNET: &str = "34.235.168.98";
pub const TIER_1_VALIDATOR_IP_PUBLIC: &str = "65.108.1.53";

pub struct NoFilter;

// Dummy filter that does nothing.
impl FilterWith<TransactionEnvelope> for NoFilter {
	fn name(&self) -> &'static str {
		"NoFilter"
	}

	fn check_for_processing(&self, _param: &TransactionEnvelope) -> bool {
		false
	}
}

// TODO: THIS WILL NOT WORK PROPERLY JUST YET, SINCE IT NEEDS SOME STUFF FROM OTHER PRS.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	prepare_directories()?;

	let args: Vec<String> = std::env::args().collect();
	let arg_network = if args.len() > 1 { &args[1] } else { "testnet" };
	let mut public_network = false;
	let mut tier1_node_ip = TIER_1_VALIDATOR_IP_TESTNET;

	if arg_network == "mainnet" {
		public_network = true;
		tier1_node_ip = TIER_1_VALIDATOR_IP_PUBLIC;
	}
	let network: &Network = if public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

	tracing::info!(
		"Connected to {:?} through {:?}",
		std::str::from_utf8(network.get_passphrase().as_slice()).unwrap(),
		tier1_node_ip
	);

	let secret =
		SecretKey::from_encoding("SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73")
			.unwrap();

	let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), network);
	let cfg = ConnConfig::new(tier1_node_ip, 11625, secret, 0, true, true, false);

	// let vault_addresses_filter =
	// 	vec!["GAP4SFKVFVKENJ7B7VORAYKPB3CJIAJ2LMKDJ22ZFHIAIVYQOR6W3CXF".to_string()];

	let handler = create_handler(node_info, cfg, public_network, vec![]).await?;

	// this is to test out that
	// 1. "retrieving envelopes from Archives" works; ✓
	// 2. "retrieving envelopes from Stellar Node" works; ✓
	// 3. todo: "retrieving transaction set" works;
	// 4.

	let mut counter = 0;

	// whether we go 10 steps back, or 100.
	let mut ten_or_hundred = 100;
	let mut get_from_random = true;

	loop {
		counter += 1;
		tokio::time::sleep(Duration::from_secs(3)).await;
		// let's try to send a message?
		let last_slot = handler.get_last_slot_index().await?;
		tracing::info!("counter: {:?}  slot: {:?}", counter, last_slot);

		// for every multiples of 10,let's get all the pending proofs.
		if counter % 10 == 0 {
			match handler.get_pending_proofs().await {
				Ok(proofs) => {
					tracing::info!("proofs size: {:?}", proofs.len());
					for proof in proofs.iter() {
						tracing::info!(
							"found proof for {:?}, with {} envelopes and {} txes",
							proof.slot(),
							proof.envelopes().len(),
							proof.tx_set().txes.len()
						);
					}
				},
				Err(e) => {
					tracing::warn!("ERROR! {:?}", e);
				},
			}
		// for every multiples of 6, let's watch out for a slot.
		} else if counter % 6 == 0 {
			// let's watch out for a slot that is 5 steps away from the current one.
			handler.watch_slot(last_slot + 5).await?;

		// for every multiples of 4, let's try to get proof of an old slot.
		} else if counter % 4 == 0 {
			let check_slot = if get_from_random {
				// let's get a proof of a slot 10 0r 100 steps away from the current one.
				let last_slot = last_slot - ten_or_hundred;
				// this helps with alternating between fetching from stellar node and from Archive..
				if ten_or_hundred == 10 {
					ten_or_hundred = 100;
				} else {
					ten_or_hundred = 10;
				}

				get_from_random = false;
				last_slot
			} else {
				get_from_random = true;
				let watch_list = handler.get_slot_watchlist().await?;

				*watch_list.first().unwrap_or(&last_slot)
			};

			match handler.get_proof(check_slot).await? {
				ProofStatus::Proof(p) => {
					tracing::info!(
						"found proof for {}, env len: {}",
						check_slot,
						p.envelopes().len()
					);
					// todo: once we figure out how to get the txset for old slots, reactivate this
					//old_slot = 0;
				},
				ProofStatus::NoTxSetFound => {
					tracing::info!(
						"skipping slot {:?}, since it's impossible to retrieve it. YET.",
						check_slot
					);
				},
				ProofStatus::WaitForTxSet => {},

				other => {
					tracing::info!("no proof yet for {}: {:?}", check_slot, other);
				},
			}
			let res = handler.get_size().await?;
			tracing::info!("  --> envelopes size: {}", res);
		}
	}
}
