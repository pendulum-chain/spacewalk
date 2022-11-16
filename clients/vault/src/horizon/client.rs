use crate::{
	error::Error,
	horizon::{HorizonTransactionsResponse, Transaction},
	oracle::ScpMessageHandler,
};
use async_trait::async_trait;
use runtime::SpacewalkPallet;
use service::Error as ServiceError;
use sp_std::{convert::From, str, vec::Vec};
use std::{sync::Arc, time::Duration};
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

struct HorizonFetcher<P: SpacewalkPallet, C: HorizonClient> {
	parachain_rpc: P,
	client: C,
	vault_secret_key: String,
	last_tx_id: Option<Vec<u8>>,
}

impl<P: SpacewalkPallet, C: HorizonClient> HorizonFetcher<P, C> {
	pub fn new(parachain_rpc: P, client: C, vault_secret_key: String) -> Self {
		Self { parachain_rpc, client, vault_secret_key, last_tx_id: None }
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
					tracing::info!(
                        "Found new transaction from Horizon (id {:#?}). Starting to process new transaction",
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
	) {
		let handler = handler.lock().await;

		let res = self.fetch_latest_txs(handler.is_public_network).await;
		let transactions = match res {
			Ok(txs) => txs._embedded.records,
			Err(e) => {
				tracing::warn!("Failed to fetch transactions: {:?}", e);
				return
			},
		};

		if transactions.len() > 0 {
			let tx = transactions[0].clone();
			let id = tx.id.clone();
			if self.is_unhandled_transaction(&tx) && is_tx_relevant(&tx, issue_set) {
				match handler.watch_transaction(tx).await {
					Ok(_) => {
						tracing::info!("following transaction {:?}", String::from_utf8(id));
					},
					Err(e) => {
						tracing::error!("Failed to watch transaction: {:?}", e);
					},
				}
			}
		}
	}
}

pub async fn poll_horizon_for_new_transactions<P: SpacewalkPallet>(
	parachain_rpc: P,
	vault_secret_key: String,
	issue_set: Arc<Mutex<IssueRequests>>,
	handler: Arc<Mutex<ScpMessageHandler>>,
) -> Result<(), ServiceError> {
	let horizon_client = reqwest::Client::new();
	let mut fetcher = HorizonFetcher::new(parachain_rpc, horizon_client, vault_secret_key);

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
