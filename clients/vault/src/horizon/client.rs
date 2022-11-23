use crate::{
	error::Error,
	horizon::{HorizonTransactionsResponse, PagingToken, Transaction},
	oracle::ScpMessageHandler,
};
use async_trait::async_trait;
use service::Error as ServiceError;
use sp_std::{convert::From, str, vec::Vec};
use std::{convert::TryInto, sync::Arc, time::Duration};
use stellar::SecretKey;
use stellar_relay::{sdk as stellar, sdk::Hash};
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
#[derive(Clone, PartialEq)]
pub struct IssueRequest {
	pub issue_id: Hash,
}

impl From<Vec<u8>> for IssueRequest {
	fn from(val: Vec<u8>) -> Self {
		IssueRequest { issue_id: val[0..31].try_into().unwrap() }
	}
}

pub type IssueRequests = Vec<IssueRequest>;

#[async_trait]
pub trait HorizonClient {
	async fn get_transactions(&self, url: &str) -> Result<HorizonTransactionsResponse, Error>;
}

#[async_trait]
impl HorizonClient for reqwest::Client {
	async fn get_transactions(&self, url: &str) -> Result<HorizonTransactionsResponse, Error> {
		self.get(url)
			.send()
			.await
			.map_err(|_| Error::HttpFetchingError)?
			.json::<HorizonTransactionsResponse>()
			.await
			.map_err(|_| Error::HttpFetchingError)
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
		next_page: PagingToken,
	) -> Result<HorizonTransactionsResponse, Error> {
		let vault_keypair: SecretKey = SecretKey::from_encoding(&self.vault_secret_key).unwrap();
		let vault_address = vault_keypair.get_public();

		let mut request_url = String::from(horizon_url(is_public_network)) +
			"/accounts/" + str::from_utf8(vault_address.to_encoding().as_slice())
			.map_err(|_| Error::HttpFetchingError)? +
			"/transactions?";

		if !next_page.is_empty() {
			request_url.push_str(&format!("cursor={}&", next_page));
		}
		request_url += "order=desc&limit=20";

		tracing::info!("request url: {:?}", request_url);

		self.client.get_transactions(request_url.as_str()).await
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
					tracing::debug!(
						"Found new transaction from Horizon (id {:#?}) to process...",
						str::from_utf8(&latest_tx_id).unwrap()
					);

					true
				} else {
					tracing::info!("Initial transaction handled");
					false
				}
			},
			Err(UP_TO_DATE) => {
				tracing::info!("Already up to date");
				false
			},
		}
	}

	async fn fetch_horizon_and_process_new_transactions(
		&mut self,
		issue_set: Arc<Mutex<IssueRequests>>,
		handler: Arc<Mutex<ScpMessageHandler>>,
		next_page: PagingToken,
	) -> Result<PagingToken, Error> {
		let handler = handler.lock().await;

		let res = self.fetch_latest_txs(handler.is_public_network, next_page).await;
		let transactions = match res {
			Ok(txs) => txs._embedded.records,
			Err(e) => {
				tracing::warn!("Failed to fetch transactions: {:?}", e);
				return Ok(PagingToken::new())
			},
		};

		let mut paging_token = PagingToken::new();
		for transaction in transactions {
			// update the paging_token
			if let Some(page) = transaction.paging_token() {
				paging_token = page;
			} else {
				paging_token = PagingToken::new();
			}

			let tx = transaction.clone();
			let id = tx.id.clone();
			if self.is_unhandled_transaction(&tx) && is_tx_relevant(&tx, issue_set.clone()).await {
				match handler.watch_slot(tx.ledger.try_into().unwrap()).await {
					Ok(_) => {
						tracing::info!("following transaction {:?}", String::from_utf8(id));
					},
					Err(e) => {
						tracing::error!("Failed to watch transaction: {:?}", e);
					},
				}
			}
		}

		Ok(paging_token)
	}
}

pub async fn poll_horizon_for_new_transactions(
	vault_secret_key: String,
	issue_set: Arc<Mutex<IssueRequests>>,
	handler: Arc<Mutex<ScpMessageHandler>>,
) -> Result<(), ServiceError> {
	let horizon_client = reqwest::Client::new();
	let mut fetcher = HorizonFetcher::new(horizon_client, vault_secret_key);

	let mut next_page = PagingToken::new();
	// Start polling horizon every 5 seconds
	loop {
		if let Ok(new_paging_token) = fetcher
			.fetch_horizon_and_process_new_transactions(
				issue_set.clone(),
				handler.clone(),
				next_page.clone(),
			)
			.await
		{
			next_page = new_paging_token;
		}
		sleep(Duration::from_millis(POLL_INTERVAL)).await;
	}
}

pub async fn is_tx_relevant(
	transaction: &Transaction,
	issue_set: Arc<Mutex<IssueRequests>>,
) -> bool {
	match String::from_utf8(transaction.memo_type.clone()) {
		Ok(memo_type) if memo_type == "hash" => {
			let issue_set = issue_set.lock().await;
			if let Some(memo) = &transaction.memo {
				return issue_set.contains(&IssueRequest::from(memo.clone()))
			}
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
		horizon::{
			client::{HorizonFetcher, POLL_INTERVAL},
			PagingToken,
		},
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
		let handler = Arc::new(Mutex::new(prepare_handler(vec![]).await));
		let mut issue_set = Arc::new(Mutex::new(vec![]));

		let horizon_client = reqwest::Client::new();
		let mut fetcher = HorizonFetcher::new(horizon_client, String::from(SECRET));

		let mut counter = 0;
		// Start polling horizon every 5 seconds

		let mut cursor = PagingToken::new();
		loop {
			println!("counter: {}", counter);
			counter += 1;
			if let Ok(next_page) = fetcher
				.fetch_horizon_and_process_new_transactions(
					issue_set.clone(),
					handler.clone(),
					cursor.clone(),
				)
				.await
			{
				cursor = next_page;
			}
			sleep(Duration::from_millis(POLL_INTERVAL)).await;

			if counter == 5 {
				break
			}
		}
	}
}
