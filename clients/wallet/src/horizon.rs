use std::{convert::TryInto, str::FromStr, sync::Arc, time::Duration};

use async_trait::async_trait;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Deserializer};
use substrate_stellar_sdk::{
	types::{AlphaNum12, AlphaNum4, AssetType},
	Asset, Hash, PublicKey, SecretKey,
};
use tokio::{
	sync::{Mutex, RwLock},
	time::sleep,
};

use crate::{error::Error, stellar_wallet::Watcher};

pub type PagingToken = u128;

// This represents each record for a transaction in the Horizon API response
#[derive(Clone, Deserialize, Encode, Decode, Default, Debug)]
pub struct Transaction {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_u128")]
	pub paging_token: PagingToken,
	successful: bool,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub hash: Vec<u8>,
	ledger: u32,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub created_at: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub source_account: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub source_account_sequence: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub fee_account: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub fee_charged: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub max_fee: Vec<u8>,
	operation_count: u32,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub envelope_xdr: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub result_xdr: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub result_meta_xdr: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub fee_meta_xdr: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub memo_type: Vec<u8>,
	#[serde(default)]
	#[serde(deserialize_with = "de_string_to_optional_bytes")]
	pub memo: Option<Vec<u8>>,
}

impl Transaction {
	pub(crate) fn ledger(&self) -> u32 {
		self.ledger
	}
}

// The following structs represent the whole response when fetching any Horizon API
// In this particular case we assume the embedded payload will allways be for transactions
// ref https://developers.stellar.org/api/introduction/response-format/
#[derive(Deserialize, Debug)]
pub struct EmbeddedTransactions {
	pub records: Vec<Transaction>,
}

#[derive(Deserialize, Debug)]
pub struct HorizonAccountResponse {
	// We don't care about specifics of pagination, so we just tell serde that this will be a
	// generic json value
	pub _links: serde_json::Value,

	#[serde(deserialize_with = "de_string_to_bytes")]
	pub id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub account_id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub sequence: Vec<u8>,
	// ...
}

#[derive(Deserialize, Debug)]
pub struct HorizonTransactionsResponse {
	// We don't care about specifics of pagination, so we just tell serde that this will be a
	// generic json value
	pub _links: serde_json::Value,
	pub _embedded: EmbeddedTransactions,
}

pub fn de_string_to_bytes<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	Ok(s.as_bytes().to_vec())
}

pub fn de_string_to_u128<'de, D>(de: D) -> Result<u128, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	u128::from_str(s).map_err(serde::de::Error::custom)
}

pub fn de_string_to_optional_bytes<'de, D>(de: D) -> Result<Option<Vec<u8>>, D::Error>
where
	D: Deserializer<'de>,
{
	Option::<&str>::deserialize(de).map(|opt_wrapped| opt_wrapped.map(|x| x.as_bytes().to_vec()))
}

// Claimable balances objects

#[derive(Deserialize, Debug)]
pub struct HorizonClaimableBalanceResponse {
	// We don't care about specifics of pagination, so we just tell serde that this will be a
	// generic json value
	pub _links: serde_json::Value,
	pub _embedded: EmbeddedClaimableBalance,
}

// The following structs represent the whole response when fetching any Horizon API
// for retreiving a list of claimable balances for an account
#[derive(Deserialize, Debug)]
pub struct EmbeddedClaimableBalance {
	pub records: Vec<ClaimableBalance>,
}

// This represents each record for a claimable balance in the Horizon API response
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct ClaimableBalance {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub paging_token: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub asset: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub amount: Vec<u8>,
	pub claimants: Vec<Claimant>,
	pub last_modified_ledger: u32,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub last_modified_time: Vec<u8>,
}

// This represents a Claimant
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct Claimant {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub destination: Vec<u8>,
	// For now we assume that the predicate is always unconditional
	// pub predicate: serde_json::Value,
}

pub const fn horizon_url(is_public_network: bool) -> &'static str {
	if is_public_network {
		"https://horizon.stellar.org"
	} else {
		"https://horizon-testnet.stellar.org"
	}
}

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

pub(crate) struct HorizonFetcher<C: HorizonClient> {
	client: C,
	is_public_network: bool,
	last_tx_id: Option<Vec<u8>>,
	vault_account_public_key: PublicKey,
}

const DEFAULT_PAGE_SIZE: u32 = 200;

impl<C: HorizonClient> HorizonFetcher<C> {
	pub fn new(client: C, vault_account_public_key: PublicKey, is_public_network: bool) -> Self {
		Self { client, vault_account_public_key, last_tx_id: None, is_public_network }
	}

	/// Fetch recent transactions from remote and deserialize to HorizonResponse
	async fn fetch_latest_txs(
		&self,
		cursor: PagingToken,
	) -> Result<HorizonTransactionsResponse, Error> {
		let mut request_url = String::from(horizon_url(self.is_public_network)) +
			"/accounts/" + std::str::from_utf8(
			self.vault_account_public_key.to_encoding().as_slice(),
		)
		.map_err(|_| Error::HttpFetchingError)? +
			"/transactions?";

		if cursor == 0 {
			// Fetch the first/latest transaction and set it as the new paging token
			request_url.push_str(&format!("order=desc&limit=1"));
		} else {
			// If we have a paging token, fetch the transactions that occurred after our last stored
			// paging token
			request_url
				.push_str(&format!("cursor={}&order=asc&limit={}", cursor, DEFAULT_PAGE_SIZE));
		}

		tracing::info!("request url: {:?}", request_url);

		self.client.get_transactions(request_url.as_str()).await
	}

	pub async fn fetch_horizon_and_process_new_transactions(
		&mut self,
		issue_hashes: Arc<RwLock<Vec<Hash>>>,
		watcher: Arc<RwLock<dyn Watcher>>,
		last_paging_token: PagingToken,
	) -> Result<PagingToken, Error> {
		let watcher = watcher.write().await;

		let res = self.fetch_latest_txs(last_paging_token).await;
		let transactions = match res {
			Ok(txs) => txs._embedded.records,
			Err(e) => {
				tracing::warn!("Failed to fetch transactions: {:?}", e);
				Vec::new()
			},
		};

		// Define the new latest paging token as the highest paging token of the transactions we
		// received or the last paging token if we didn't receive any transactions
		let latest_paging_token = transactions
			.iter()
			.map(|tx| tx.paging_token)
			.max()
			.unwrap_or_else(|| last_paging_token);

		let issue_hashes = issue_hashes.read().await;
		for transaction in transactions {
			let tx = transaction.clone();
			let id = tx.id.clone();
			if is_tx_relevant(&tx, issue_hashes.clone()).await {
				match watcher.watch_slot(tx.ledger.try_into().unwrap()).await {
					Ok(_) => {
						tracing::info!("following transaction {:?}", String::from_utf8(id));
					},
					Err(e) => {
						tracing::error!("Failed to watch transaction: {:?}", e);
					},
				}
			}
		}

		Ok(latest_paging_token)
	}
}

async fn is_tx_relevant(transaction: &Transaction, issue_hashes: Vec<Hash>) -> bool {
	match String::from_utf8(transaction.memo_type.clone()) {
		Ok(memo_type) if memo_type == "hash" =>
			if let Some(memo) = &transaction.memo {
				return issue_hashes.iter().any(|hash| &hash.to_vec() == memo)
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
	use std::{sync::Arc, time::Duration};

	use tokio::{sync::Mutex, time::sleep};

	use super::*;

	const SECRET: &'static str = "SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73";

	struct MockWatcher;

	#[async_trait]
	impl Watcher for MockWatcher {
		async fn watch_slot(&self, slot: u128) -> Result<(), Error> {
			todo!()
		}
	}

	// not yet working
	#[tokio::test(flavor = "multi_thread")]
	async fn client_test_for_polling() {
		let watcher = Arc::new(RwLock::new(MockWatcher));
		let issue_hashes = Arc::new(RwLock::new(vec![]));

		let horizon_client = reqwest::Client::new();
		let secret = SecretKey::from_encoding(SECRET).unwrap();
		let mut fetcher = HorizonFetcher::new(horizon_client, secret.get_public().clone(), false);

		let mut counter = 0;
		// Start polling horizon every 5 seconds

		let mut cursor = 0;
		loop {
			println!("counter: {}", counter);
			counter += 1;
			if let Ok(next_page) = fetcher
				.fetch_horizon_and_process_new_transactions(
					issue_hashes.clone(),
					watcher.clone(),
					cursor,
				)
				.await
			{
				cursor = next_page;
			}
			sleep(Duration::from_millis(5000)).await;

			if counter == 5 {
				break
			}
		}

		// TODO assert some stuff
	}
}
