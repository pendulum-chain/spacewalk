use std::{str::FromStr, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::future;
use parity_scale_codec::{Decode, Encode};
use primitives::TextMemo;
use serde::{de::DeserializeOwned, Deserialize, Deserializer};
use substrate_stellar_sdk::{PublicKey, TransactionEnvelope, XdrCodec};
use tokio::{sync::RwLock, time::sleep};

use crate::{
	error::Error,
	types::{FilterWith, PagingToken},
	LedgerTxEnvMap, StellarWallet,
};

// todo: change to Slot
pub type Ledger = u32;

const POLL_INTERVAL: u64 = 5000;
pub const TXS_LIMIT_PER_PAGE: i64 = 500;

/// Interprets the response from Horizon into something easier to read.
async fn interpret_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, Error> {
	if response.status().is_success() {
		return response.json::<T>().await.map_err(Error::HttpFetchingError)
	}

	let resp = response.json::<serde_json::Value>().await.map_err(Error::HttpFetchingError)?;

	let unknown = "unknown";
	let title = resp["title"].as_str().unwrap_or(unknown);
	let status = u16::try_from(resp["status"].as_u64().unwrap_or(400)).unwrap_or(400);

	let error = match status {
		400 => {
			let envelope_xdr = resp["extras"]["envelope_xdr"].as_str().unwrap_or(unknown);

			match title.to_lowercase().as_str() {
				// this particular status does not have the "result_code",
				// so the "detail" portion will be used for "reason".
				"transaction malformed" => {
					let detail = resp["detail"].as_str().unwrap_or(unknown);

					Error::HorizonSubmissionError {
						title: title.to_string(),
						status,
						reason: detail.to_string(),
						envelope_xdr: Some(envelope_xdr.to_string()),
					}
				},
				_ => {
					let result_code =
						resp["extras"]["result_codes"]["transaction"].as_str().unwrap_or(unknown);

					Error::HorizonSubmissionError {
						title: title.to_string(),
						status,
						reason: result_code.to_string(),
						envelope_xdr: Some(envelope_xdr.to_string()),
					}
				},
			}
		},
		_ => {
			let detail = resp["detail"].as_str().unwrap_or(unknown);

			Error::HorizonSubmissionError {
				title: title.to_string(),
				status,
				reason: detail.to_string(),
				envelope_xdr: None,
			}
		},
	};

	tracing::error!("Response returned error: {:?}", &error);
	Err(error)
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

pub fn de_string_to_u64<'de, D>(de: D) -> Result<u64, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	u64::from_str(s).map_err(serde::de::Error::custom)
}

pub fn de_string_to_i64<'de, D>(de: D) -> Result<i64, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	i64::from_str(s).map_err(serde::de::Error::custom)
}

pub fn de_string_to_f64<'de, D>(de: D) -> Result<f64, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	f64::from_str(s).map_err(serde::de::Error::custom)
}

pub fn de_string_to_optional_bytes<'de, D>(de: D) -> Result<Option<Vec<u8>>, D::Error>
where
	D: Deserializer<'de>,
{
	Option::<&str>::deserialize(de).map(|opt_wrapped| opt_wrapped.map(|x| x.as_bytes().to_vec()))
}

// The following structs represent the whole response when fetching any Horizon API
// In this particular case we assume the embedded payload will allways be for transactions
// ref https://developers.stellar.org/api/introduction/response-format/
#[derive(Deserialize, Debug)]
pub struct HorizonTransactionsResponse {
	pub _embedded: EmbeddedTransactions,
}

#[derive(Deserialize, Debug)]
pub struct EmbeddedTransactions {
	pub records: Vec<TransactionResponse>,
}

// This represents each record for a transaction in the Horizon API response
#[derive(Clone, Deserialize, Encode, Decode, Default, Debug)]
pub struct TransactionResponse {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_u128")]
	pub paging_token: PagingToken,
	pub successful: bool,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub hash: Vec<u8>,
	pub ledger: Ledger,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub created_at: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub source_account: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub source_account_sequence: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub fee_account: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_u64")]
	pub fee_charged: u64,
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

#[allow(dead_code)]
impl TransactionResponse {
	pub(crate) fn ledger(&self) -> Ledger {
		self.ledger
	}

	pub fn memo_text(&self) -> Option<&TextMemo> {
		if self.memo_type == b"text" {
			self.memo.as_ref()
		} else {
			None
		}
	}

	pub fn to_envelope(&self) -> Result<TransactionEnvelope, Error> {
		TransactionEnvelope::from_base64_xdr(self.envelope_xdr.clone())
			.map_err(|_| Error::DecodeError)
	}
}

#[derive(Deserialize, Debug)]
pub struct HorizonAccountResponse {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub account_id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_i64")]
	pub sequence: i64,
	pub balances: Vec<Balance>,
	// ...
}

// This represents a Claimant
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct Balance {
	#[serde(deserialize_with = "de_string_to_f64")]
	pub balance: f64,
	#[serde(default)]
	#[serde(deserialize_with = "de_string_to_optional_bytes")]
	pub asset_code: Option<Vec<u8>>,
	#[serde(default)]
	#[serde(deserialize_with = "de_string_to_optional_bytes")]
	pub asset_issuer: Option<Vec<u8>>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub asset_type: Vec<u8>,
}

#[derive(Deserialize, Debug)]
pub struct HorizonClaimableBalanceResponse {
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
	pub last_modified_ledger: Ledger,
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
	async fn get_transactions(
		&self,
		account_id: &str,
		is_public_network: bool,
		cursor: PagingToken,
		limit: i64,
		order_ascending: bool,
	) -> Result<HorizonTransactionsResponse, Error>;
	async fn get_account(
		&self,
		account_encoded: &str,
		is_public_network: bool,
	) -> Result<HorizonAccountResponse, Error>;
	async fn submit_transaction(
		&self,
		transaction: TransactionEnvelope,
		is_public_network: bool,
	) -> Result<TransactionResponse, Error>;
}

#[async_trait]
impl HorizonClient for reqwest::Client {
	async fn get_transactions(
		&self,
		account_id: &str,
		is_public_network: bool,
		cursor: PagingToken,
		limit: i64,
		order_ascending: bool,
	) -> Result<HorizonTransactionsResponse, Error> {
		let base_url = horizon_url(is_public_network);
		let mut url = format!("{}/accounts/{}/transactions", base_url, account_id);

		if limit != 0 {
			url = format!("{}?limit={}", url, limit);
		} else {
			url = format!("{}?limit={}", url, DEFAULT_PAGE_SIZE);
		}

		if cursor != 0 {
			url = format!("{}&cursor={}", url, cursor);
		}

		if order_ascending {
			url = format!("{}&order=asc", url);
		} else {
			url = format!("{}&order=desc", url);
		}

		let response = self.get(url).send().await.map_err(Error::HttpFetchingError)?;
		interpret_response::<HorizonTransactionsResponse>(response).await
	}

	async fn get_account(
		&self,
		account_encoded: &str,
		is_public_network: bool,
	) -> Result<HorizonAccountResponse, Error> {
		let base_url = horizon_url(is_public_network);
		let url = format!("{}/accounts/{}", base_url, account_encoded);

		let response = self.get(url).send().await.map_err(Error::HttpFetchingError)?;

		interpret_response::<HorizonAccountResponse>(response).await
	}

	async fn submit_transaction(
		&self,
		transaction_envelope: TransactionEnvelope,
		is_public_network: bool,
	) -> Result<TransactionResponse, Error> {
		let transaction_xdr = transaction_envelope.to_base64_xdr();
		let transaction_xdr = std::str::from_utf8(&transaction_xdr).map_err(Error::Utf8Error)?;

		let base_url = horizon_url(is_public_network);
		let url = format!("{}/transactions", base_url);

		let params = [("tx", &transaction_xdr)];
		let response =
			self.post(url).form(&params).send().await.map_err(Error::HttpFetchingError)?;

		interpret_response::<TransactionResponse>(response).await.map_err(|e| match e {
			Error::HorizonSubmissionError { title, status, reason, envelope_xdr }
				if envelope_xdr.is_none() =>
			{
				// let's add a transaction envelope, if possible
				Error::HorizonSubmissionError {
					title,
					status,
					reason,
					envelope_xdr: Some(transaction_xdr.to_string()),
				}
			},
			other => other,
		})
	}
}

pub(crate) struct HorizonFetcher<C: HorizonClient> {
	client: C,
	is_public_network: bool,
	vault_account_public_key: PublicKey,
}

const DEFAULT_PAGE_SIZE: i64 = 200;

impl<C: HorizonClient> HorizonFetcher<C> {
	pub fn new(client: C, vault_account_public_key: PublicKey, is_public_network: bool) -> Self {
		Self { client, vault_account_public_key, is_public_network }
	}

	/// Fetch recent transactions from remote and deserialize to HorizonResponse
	async fn fetch_latest_txs(
		&self,
		cursor: PagingToken,
	) -> Result<HorizonTransactionsResponse, Error> {
		let public_key_encoded = self.vault_account_public_key.to_encoding();
		let account_id = std::str::from_utf8(&public_key_encoded).map_err(Error::Utf8Error)?;

		if cursor == 0 {
			// Fetch the first/latest transaction and set it as the new paging token
			self.client
				.get_transactions(account_id, self.is_public_network, 0, 1, false)
				.await
		} else {
			// If we have a paging token, fetch the transactions that occurred after our last stored
			// paging token
			self.client
				.get_transactions(
					account_id,
					self.is_public_network,
					cursor,
					DEFAULT_PAGE_SIZE,
					true,
				)
				.await
		}
	}

	/// Fetches the transactions from horizon
	///
	/// # Arguments
	/// * `ledger_env_map` -  a list of TransactionEnvelopes and its corresponding ledger it belongs
	///   to
	/// * `targets` - helps in filtering out the transactions to save
	/// * `is_public_network` - the network the transaction belongs to
	/// * `filter` - logic to save the needed transaction
	/// * `last_paging_token` - acts as a marker to scroll over sets of transactions
	pub async fn fetch_horizon_and_process_new_transactions<T: Clone, U: Clone>(
		&mut self,
		ledger_env_map: Arc<RwLock<LedgerTxEnvMap>>,
		issue_map: Arc<RwLock<T>>,
		memos_to_issue_ids: Arc<RwLock<U>>,
		filter: impl FilterWith<T, U>,
		last_paging_token: PagingToken,
	) -> PagingToken {
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
		let latest_paging_token =
			transactions.iter().map(|tx| tx.paging_token).max().unwrap_or(last_paging_token);

		let (issue_map, memos_to_issue_ids) =
			future::join(issue_map.read(), memos_to_issue_ids.read()).await;
		for transaction in transactions {
			let tx = transaction.clone();

			if filter.is_relevant(tx.clone(), &issue_map, &memos_to_issue_ids) {
				tracing::info!(
					"Adding transaction {:?} with slot {} to the ledger_env_map",
					String::from_utf8(tx.id.clone()),
					tx.ledger
				);
				if let Ok(tx_env) = tx.to_envelope() {
					ledger_env_map.write().await.insert(tx.ledger, tx_env);
				}
			}
		}

		latest_paging_token
	}
}

///  Saves transactions in the map, based on the filter and the kind of filter
///
/// # Arguments
///
/// * `vault_account_public_key` - used to get the transaction
/// * `is_public_network` - the network the transaction belongs to
/// * `ledger_env_map` -  a list of TransactionEnvelopes and its corresponding ledger it belongs to
/// * `targets` - helps in filtering out the transactions to save
/// * `filter` - logic to save the needed transaction
pub async fn listen_for_new_transactions<T, U, Filter>(
	stellar_wallet: Arc<RwLock<StellarWallet>>,
	ledger_env_map: Arc<RwLock<LedgerTxEnvMap>>,
	issue_map: Arc<RwLock<T>>,
	memos_to_issue_ids: Arc<RwLock<U>>,
	filter: Filter,
) -> Result<(), Error>
where
	T: Clone,
	U: Clone,
	Filter: FilterWith<T, U> + Clone,
{
	let wallet = stellar_wallet.read().await;
	let vault_account_public_key = wallet.get_public_key();
	let is_public_network = wallet.is_public_network();

	let horizon_client = reqwest::Client::new();
	let mut fetcher =
		HorizonFetcher::new(horizon_client, vault_account_public_key, is_public_network);

	let mut latest_paging_token: PagingToken = wallet.get_last_cursor();

	loop {
		latest_paging_token = fetcher
			.fetch_horizon_and_process_new_transactions(
				ledger_env_map.clone(),
				issue_map.clone(),
				memos_to_issue_ids.clone(),
				filter.clone(),
				latest_paging_token,
			)
			.await;

		// do not throw an error if it fails to save.
		if let Err(e) = wallet.save_cursor(latest_paging_token) {
			tracing::warn!("failed to save paging token {} as cursor:{e:?}", latest_paging_token);
		}

		sleep(Duration::from_millis(POLL_INTERVAL)).await;
	}
}

#[cfg(test)]
mod tests {
	use std::{collections::HashMap, sync::Arc};

	use mockall::predicate::*;
	use substrate_stellar_sdk::{
		network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
		types::Preconditions,
		Asset, Operation, SecretKey, StroopAmount, Transaction, TransactionEnvelope,
	};

	use crate::types::{FilterWith, TransactionResponse};

	use super::*;

	const SECRET: &str = "SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73";

	#[derive(Clone)]
	struct MockFilter;

	impl FilterWith<Vec<u64>, Vec<u64>> for MockFilter {
		fn is_relevant(
			&self,
			_response: TransactionResponse,
			_param_t: &Vec<u64>,
			_param_u: &Vec<u64>,
		) -> bool {
			// We consider all transactions relevant for the test
			true
		}
	}

	async fn build_simple_transaction(
		source: SecretKey,
		destination: PublicKey,
		amount: i64,
		is_public_network: bool,
	) -> Result<TransactionEnvelope, Error> {
		let horizon_client = reqwest::Client::new();

		let public_key_encoded = source.get_encoded_public();
		let account_id_string =
			std::str::from_utf8(&public_key_encoded).map_err(Error::Utf8Error)?;
		let account = horizon_client.get_account(account_id_string, is_public_network).await?;
		let next_sequence_number = account.sequence + 1;

		let fee_per_operation = 100;

		let mut transaction = Transaction::new(
			source.get_public().clone(),
			next_sequence_number,
			Some(fee_per_operation),
			Preconditions::PrecondNone,
			None,
		)
		.map_err(|_e| {
			Error::BuildTransactionError("Creating new transaction failed".to_string())
		})?;

		let asset = Asset::native();
		let amount = StroopAmount(amount);
		transaction
			.append_operation(
				Operation::new_payment(destination, asset, amount)
					.map_err(|_e| {
						Error::BuildTransactionError(
							"Creation of payment operation failed".to_string(),
						)
					})?
					.set_source_account(source.get_public().clone())
					.map_err(|_e| {
						Error::BuildTransactionError("Setting source account failed".to_string())
					})?,
			)
			.map_err(|_e| {
				Error::BuildTransactionError("Appending payment operation failed".to_string())
			})?;

		let mut envelope = transaction.into_transaction_envelope();
		let network: &Network = if is_public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

		envelope.sign(network, vec![&source]).expect("Signing failed");

		Ok(envelope)
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn horizon_submit_transaction_success() {
		let horizon_client = reqwest::Client::new();

		let source = SecretKey::from_encoding(SECRET).unwrap();
		// The destination is the same account as the source
		let destination = source.get_public().clone();
		let amount = 100;

		// Build simple transaction
		let tx_env = build_simple_transaction(source, destination, amount, false)
			.await
			.expect("Failed to build transaction");

		match horizon_client.submit_transaction(tx_env, false).await {
			Ok(res) => {
				assert!(res.successful);
				assert!(res.ledger > 0);
			},
			Err(e) => {
				panic!("failed: {:?}", e);
			},
		}
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn horizon_get_account_success() {
		let horizon_client = reqwest::Client::new();

		let public_key_encoded = "GAYOLLLUIZE4DZMBB2ZBKGBUBZLIOYU6XFLW37GBP2VZD3ABNXCW4BVA";
		match horizon_client.get_account(public_key_encoded, true).await {
			Ok(res) => {
				let res_account = std::str::from_utf8(&res.account_id).unwrap();
				assert_eq!(res_account, public_key_encoded);
				assert!(res.sequence > 0);
			},
			Err(e) => {
				panic!("failed: {:?}", e);
			},
		}
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn horizon_get_transaction_success() {
		let horizon_client = reqwest::Client::new();

		let public_key_encoded = "GAYOLLLUIZE4DZMBB2ZBKGBUBZLIOYU6XFLW37GBP2VZD3ABNXCW4BVA";
		let limit = 2;
		match horizon_client.get_transactions(public_key_encoded, true, 0, limit, false).await {
			Ok(res) => {
				let txs = res._embedded.records;
				assert_eq!(txs.len(), 2);
			},
			Err(e) => {
				panic!("failed: {:?}", e);
			},
		}
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn horizon_fetch_txs_cursor() {
		let horizon_client = reqwest::Client::new();
		let secret = SecretKey::from_encoding(SECRET).unwrap();
		let fetcher = HorizonFetcher::new(horizon_client, secret.get_public().clone(), false);

		let res = fetcher.fetch_latest_txs(0).await.expect("should return a response");
		let txs = res._embedded.records;
		let latest_paging_token = txs.iter().map(|tx| tx.paging_token).max().unwrap_or(0);

		assert_ne!(latest_paging_token, 0);
		let res = fetcher
			.fetch_latest_txs(latest_paging_token)
			.await
			.expect("should return a response");
		// assert that when using the latest paging token for fetching we don't get any new
		// transactions
		assert_eq!(res._embedded.records.len(), 0);
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn client_test_for_polling() {
		let issue_hashes = Arc::new(RwLock::new(vec![]));
		let memos_to_issue_ids = Arc::new(RwLock::new(vec![]));
		let slot_env_map = Arc::new(RwLock::new(HashMap::new()));

		let horizon_client = reqwest::Client::new();
		let secret = SecretKey::from_encoding(SECRET).unwrap();
		let mut fetcher = HorizonFetcher::new(horizon_client, secret.get_public().clone(), false);

		assert!(slot_env_map.read().await.is_empty());

		let cursor = fetcher
			.fetch_horizon_and_process_new_transactions(
				slot_env_map.clone(),
				issue_hashes.clone(),
				memos_to_issue_ids.clone(),
				MockFilter,
				0u128,
			)
			.await;

		fetcher
			.fetch_horizon_and_process_new_transactions(
				slot_env_map.clone(),
				issue_hashes.clone(),
				memos_to_issue_ids.clone(),
				MockFilter,
				cursor,
			)
			.await;

		assert!(!slot_env_map.read().await.is_empty());
	}
}
