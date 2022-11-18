use crate::{
	error::Error,
	horizon::{HorizonTransactionsResponse, Transaction},
	oracle::ScpMessageHandler,
};
use async_trait::async_trait;
use runtime::SpacewalkPallet;
use service::Error as ServiceError;
use sp_std::{convert::From, str, vec::Vec};
use std::{convert::TryInto, sync::Arc, time::Duration};
use stellar::SecretKey;
use stellar_relay::{
	sdk as stellar,
	sdk::{Hash, PublicKey},
};
use tokio::{sync::Mutex, time::sleep};

const POLL_INTERVAL: u64 = 5000;

pub const fn horizon_url(is_public_network: bool) -> &'static str {
	if is_public_network {
		"https://horizon.stellar.org"
	} else {
		"https://horizon-testnet.stellar.org"
	}
}

// todo: temporary struct, as the real IssueRequest definition is in another branch.
#[derive(Clone)]
pub struct IssueRequest {
	pub issue_id: Hash,
}

pub type IssueRequests = Vec<IssueRequest>;

#[async_trait]
pub trait HorizonClient {
	async fn get_transactions(&self, url: &str) -> Result<HorizonTransactionsResponse, Error>;
}

#[async_trait]
impl HorizonClient for reqwest::Client {
	async fn get_transactions(&self, url: &str) -> Result<HorizonTransactionsResponse, Error> {
		let response = self
			.get(url)
			.send()
			.await
			.map_err(|_| Error::HttpFetchingError)?
			.json::<HorizonTransactionsResponse>()
			.await
			.map_err(|_| Error::HttpFetchingError)?;

		Ok(response)
	}
}

struct HorizonFetcher<C: HorizonClient> {
	client: C,
	vault_secret_key: String,
	last_tx_id: Option<Vec<u8>>,
}

impl<C: HorizonClient> HorizonFetcher<C> {
	pub fn new(client: C, vault_secret_key: String) -> Self {
		Self { client, vault_secret_key, last_tx_id: None }
	}

	/// Fetch recent transactions from remote and deserialize to HorizonResponse
	/// Since the limit in the request url is set to one it will always fetch just one
	async fn fetch_latest_txs(
		&self,
		is_public_network: bool,
	) -> Result<HorizonTransactionsResponse, Error> {
		let vault_keypair: SecretKey = SecretKey::from_encoding(&self.vault_secret_key).unwrap();
		let vault_address = vault_keypair.get_public();

		let request_url = String::from(horizon_url(is_public_network)) +
			"/accounts/" + str::from_utf8(vault_address.to_encoding().as_slice())
			.map_err(|_| Error::HttpFetchingError)? +
			"/transactions?order=desc&limit=1";

		println!("request url: {:?}", request_url);

		let horizon_response = self.client.get_transactions(request_url.as_str()).await;

		horizon_response
	}

	fn is_unhandled_transaction(&mut self, tx: &Transaction) -> bool {
		const UP_TO_DATE: () = ();
		let latest_tx_id_utf8 = &tx.id;

		let prev_tx_id = &self.last_tx_id;
		let initial = !matches!(prev_tx_id, Some(_));

		let result = match prev_tx_id {
			Some(prev_tx_id) =>
				if prev_tx_id == latest_tx_id_utf8 {
					Err(UP_TO_DATE)
				} else {
					Ok(latest_tx_id_utf8.clone())
				},
			None => Ok(latest_tx_id_utf8.clone()),
		};

		match result {
			Ok(latest_tx_id) => {
				self.last_tx_id = Some(latest_tx_id.clone());
				if !initial {
					println!(
                        "Found new transaction from Horizon (id {:#?}). Starting to process new transaction",
                        str::from_utf8(&latest_tx_id).unwrap()
                    );
					tracing::info!(
                        "Found new transaction from Horizon (id {:#?}). Starting to process new transaction",
                        str::from_utf8(&latest_tx_id).unwrap()
                    );

					true
				} else {
					println!("Initial transaction handled");
					tracing::info!("Initial transaction handled");
					false
				}
			},
			Err(UP_TO_DATE) => {
				println!("Already up to date");
				tracing::info!("Already up to date");
				false
			},
		}
	}

	async fn fetch_horizon_and_process_new_transactions(
		&mut self,
		issue_set: Arc<Mutex<IssueRequests>>,
		handler: Arc<Mutex<ScpMessageHandler>>,
	) {
		let handler = handler.lock().await;

		let res = self.fetch_latest_txs(handler.is_public_network).await;
		let transactions = match res {
			Ok(txs) => txs._embedded.records,
			Err(e) => {
				tracing::warn!("Failed to fetch transactions: {:?}", e);
				println!("oh no, :( failed to fetch transactions: {:?}", e);
				return
			},
		};

		if transactions.len() > 0 {
			let tx = transactions[0].clone();
			let id = tx.id.clone();
			if self.is_unhandled_transaction(&tx) && is_tx_relevant(&tx, issue_set) {
				match handler.watch_slot(tx.ledger.try_into().unwrap()).await {
					Ok(_) => {
						println!("following transaction {:?}", String::from_utf8(id.clone()));
						tracing::info!("following transaction {:?}", String::from_utf8(id));
					},
					Err(e) => {
						println!("Failed to watch transaction: {:?}", e);
						tracing::error!("Failed to watch transaction: {:?}", e);
					},
				}
			}
		}
	}
}

pub async fn poll_horizon_for_new_transactions(
	vault_secret_key: String,
	issue_set: Arc<Mutex<IssueRequests>>,
	handler: Arc<Mutex<ScpMessageHandler>>,
) -> Result<(), ServiceError> {
	let horizon_client = reqwest::Client::new();
	let mut fetcher = HorizonFetcher::new(horizon_client, vault_secret_key);

	// Start polling horizon every 5 seconds
	loop {
		fetcher
			.fetch_horizon_and_process_new_transactions(issue_set.clone(), handler.clone())
			.await;
		sleep(Duration::from_millis(POLL_INTERVAL)).await;
	}
}

fn is_tx_relevant(transaction: &Transaction, _issue_set: Arc<Mutex<IssueRequests>>) -> bool {
	match String::from_utf8(transaction.memo_type.clone()) {
		Ok(memo_type) if memo_type == "hash" => {
			// we only want those with hash memo type.
			// todo: check if the memo == to any of the issue_id in the issue_set
			return true
		},
		Err(e) => {
			tracing::error!("Failed to retrieve memo type: {:?}", e);
		},
		_ => {},
	}

	false
}

#[cfg(test)]
mod tests {
	use crate::{
		horizon::client::{HorizonFetcher, POLL_INTERVAL},
		oracle::{create_handler, ScpMessageHandler},
		system::TIER_1_VALIDATOR_IP_PUBLIC,
	};
	use std::{sync::Arc, time::Duration};
	use stellar_relay::{
		node::NodeInfo,
		sdk::{network::PUBLIC_NETWORK, SecretKey},
		ConnConfig, StellarOverlayConnection,
	};
	use tokio::{sync::Mutex, time::sleep};

	const SECRET: &'static str = "SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73";

	async fn prepare_handler(vault_addresses: Vec<String>) -> ScpMessageHandler {
		let secret = SecretKey::from_encoding(SECRET).unwrap();
		let node_info = NodeInfo::new(19, 25, 23, "v19.5.0".to_string(), &PUBLIC_NETWORK);
		let cfg = ConnConfig::new(TIER_1_VALIDATOR_IP_PUBLIC, 11625, secret, 0, false, true, false);
		create_handler(node_info, cfg, true, vault_addresses).await.unwrap()
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn client_test_for_polling() {
		println!("RUNNING TEST: ");
		let handler = Arc::new(Mutex::new(prepare_handler(vec![]).await));
		let mut issue_set = Arc::new(Mutex::new(vec![]));

		let horizon_client = reqwest::Client::new();
		let mut fetcher = HorizonFetcher::new(horizon_client, String::from(SECRET));

		let mut counter = 0;
		// Start polling horizon every 5 seconds
		loop {
			println!("counter: {}", counter);
			counter += 1;
			fetcher
				.fetch_horizon_and_process_new_transactions(issue_set.clone(), handler.clone())
				.await;
			sleep(Duration::from_millis(POLL_INTERVAL)).await;

			if counter == 5 {
				break
			}
		}
	}
}
