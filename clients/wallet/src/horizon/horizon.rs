use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::{
	future::{self, ready},
	TryFutureExt,
};

use primitives::{
	stellar::{ClaimableBalanceId, PublicKey, TransactionEnvelope, XdrCodec},
	StellarTypeToString, TransactionEnvelopeExt,
};
use rand::seq::SliceRandom;
use serde::de::DeserializeOwned;
use tokio::{sync::RwLock, time::sleep};

use crate::{
	error::Error,
	horizon::{
		responses::{
			interpret_response, HorizonAccountResponse, HorizonClaimableBalanceResponse,
			HorizonTransactionsResponse, TransactionResponse, TransactionsResponseIter,
		},
		traits::HorizonClient,
	},
	types::{FilterWith, PagingToken},
	LedgerTxEnvMap,
};

const POLL_INTERVAL: u64 = 5000;
/// See [Stellar doc](https://developers.stellar.org/api/introduction/pagination/page-arguments)
pub const DEFAULT_PAGE_SIZE: u8 = 200;

pub fn horizon_url(is_public_network: bool, is_need_fallback: bool) -> &'static str {
	if is_public_network {
		if is_need_fallback {
			let other_urls =
				vec!["https://horizon.stellarx.com", "https://horizon.stellar.lobstr.co"];

			return other_urls
				.choose(&mut rand::thread_rng())
				.unwrap_or(&"https://horizon.stellar.org")
		}

		"https://horizon.stellar.org"
	} else {
		"https://horizon-testnet.stellar.org"
	}
}

#[async_trait]
impl HorizonClient for reqwest::Client {
	async fn get_from_url<R: DeserializeOwned>(&self, url: &str) -> Result<R, Error> {
		tracing::debug!("accessing url: {url:?}");
		let response = self.get(url).send().await.map_err(Error::HorizonResponseError)?;
		interpret_response::<R>(response).await
	}

	async fn get_account_transactions<A: StellarTypeToString<PublicKey, Error> + Send>(
		&self,
		account_id: A,
		is_public_network: bool,
		cursor: PagingToken,
		limit: u8,
		order_ascending: bool,
	) -> Result<HorizonTransactionsResponse, Error> {
		let account_id_encoded = account_id.as_encoded_string()?;

		let base_url = horizon_url(is_public_network, false);
		let mut url = format!("{}/accounts/{}/transactions", base_url, account_id_encoded);

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

		self.get_from_url(&url).await
	}

	async fn get_account<A: StellarTypeToString<PublicKey, Error> + Send>(
		&self,
		account_id: A,
		is_public_network: bool,
	) -> Result<HorizonAccountResponse, Error> {
		let account_id_encoded = account_id.as_encoded_string()?;
		let base_url = horizon_url(is_public_network, false);
		let url = format!("{}/accounts/{}", base_url, account_id_encoded);

		self.get_from_url(&url).await
	}

	async fn get_claimable_balance<A: StellarTypeToString<ClaimableBalanceId, Error> + Send>(
		&self,
		claimable_balance_id: A,
		is_public_network: bool,
	) -> Result<HorizonClaimableBalanceResponse, Error> {
		let id_encoded = claimable_balance_id.as_encoded_string()?;
		let base_url = horizon_url(is_public_network, false);
		let url = format!("{}/claimable_balances/{}", base_url, id_encoded);

		self.get_from_url(&url).await
	}

	async fn submit_transaction(
		&self,
		transaction_envelope: TransactionEnvelope,
		is_public_network: bool,
		max_retries: u8,
		max_backoff_delay_in_secs: u16,
	) -> Result<TransactionResponse, Error> {
		let seq_no = transaction_envelope.sequence_number();

		tracing::debug!("submitting transaction with seq no: {seq_no:?}: {transaction_envelope:?}");

		let transaction_xdr = transaction_envelope.to_base64_xdr();
		let transaction_xdr = std::str::from_utf8(&transaction_xdr).map_err(Error::Utf8Error)?;

		let params = [("tx", &transaction_xdr)];

		let mut server_error_count = 0;
		let mut exponent_counter = 1;

		loop {
			let need_fallback = if server_error_count == max_retries {
				// reset in case the fallback fails but original url could be running again.
				server_error_count = 0;
				true
			} else {
				false
			};

			let base_url = horizon_url(is_public_network, need_fallback);
			let url = format!("{}/transactions", base_url);

			let response = ready(
				self.post(url).form(&params).send().await.map_err(Error::HorizonResponseError),
			)
			.and_then(|response| async move {
				interpret_response::<TransactionResponse>(response).await
			})
			.await;

			match response {
				Err(e) if e.is_recoverable() || e.is_server_error() => {
					if e.is_server_error() {
						server_error_count += 1;
					}

					// let's wait awhile before resubmitting.
					tracing::warn!(
						"submitting transaction with seq no: {seq_no:?} failed with {e:?}"
					);
					// exponentially sleep before retrying again
					let sleep_duration = 2u64.pow(exponent_counter);
					sleep(Duration::from_secs(sleep_duration)).await;

					// retry/resubmit again
					if sleep_duration < u64::from(max_backoff_delay_in_secs) {
						exponent_counter += 1;
					}
					continue
				},

				Err(Error::HorizonSubmissionError { title, status, reason, envelope_xdr }) => {
					tracing::error!("submitting transaction with seq no: {seq_no:?}: failed with {title}, {reason}");
					tracing::debug!("submitting transaction with seq no: {seq_no:?}: the envelope: {envelope_xdr:?}");
					let envelope_xdr = envelope_xdr.or(Some(transaction_xdr.to_string()));
					// let's add a transaction envelope, if possible
					return Err(Error::HorizonSubmissionError {
						title,
						status,
						reason,
						envelope_xdr,
					})
				},

				other => return other,
			}
		}
	}
}

pub(crate) struct HorizonFetcher<C: HorizonClient> {
	client: C,
	is_public_network: bool,
	vault_account_public_key: PublicKey,
}

impl<C: HorizonClient + Clone> HorizonFetcher<C> {
	#[allow(dead_code)]
	pub fn new(client: C, vault_account_public_key: PublicKey, is_public_network: bool) -> Self {
		Self { client, vault_account_public_key, is_public_network }
	}

	/// Returns an iter for a list of transactions.
	/// This method is LOOKING FORWARD, so the list is in ASCENDING order:
	/// starting from the oldest ones, or depending on the last cursor
	pub(crate) async fn fetch_transactions_iter(
		&self,
		last_cursor: PagingToken,
	) -> Result<TransactionsResponseIter<C>, Error> {
		let transactions_response = self
			.client
			.get_account_transactions(
				self.vault_account_public_key.to_encoding(),
				self.is_public_network,
				last_cursor,
				DEFAULT_PAGE_SIZE,
				true,
			)
			.await?;

		let next_page = transactions_response.next_page();
		let records = transactions_response.records();

		Ok(TransactionsResponseIter { records, next_page, client: self.client.clone() })
	}

	/// Fetches the transactions from horizon
	///
	/// # Arguments
	/// * `ledger_env_map` -  list of TransactionEnvelopes and the ledger it belongs to
	/// * `targets` - helps in filtering out the transactions to save
	/// * `is_public_network` - the network the transaction belongs to
	/// * `filter` - logic to save the needed transaction
	pub async fn fetch_horizon_and_process_new_transactions<T: Clone, U: Clone>(
		&mut self,
		ledger_env_map: Arc<RwLock<LedgerTxEnvMap>>,
		issue_map: Arc<RwLock<T>>,
		memos_to_issue_ids: Arc<RwLock<U>>,
		filter: impl FilterWith<T, U>,
		last_cursor: PagingToken,
	) -> Result<PagingToken, Error> {
		let mut last_cursor = last_cursor;

		let mut txs_iter = self.fetch_transactions_iter(last_cursor).await?;

		let (issue_map, memos_to_issue_ids) =
			future::join(issue_map.read(), memos_to_issue_ids.read()).await;

		while let Some(tx) = txs_iter.next().await {
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

			if txs_iter.is_empty() {
				// save the last cursor and the last sequence found.
				last_cursor = tx.paging_token;
			}
		}

		Ok(last_cursor)
	}
}

///  Saves transactions in the map, based on the filter and the kind of filter
///
/// # Arguments
///
/// * `vault_account_public_key` - used to get the transaction
/// * `is_public_network` - the network the transaction belongs to
/// * `last_cursor` - the last page known, containing the latest transactions
/// * `ledger_env_map` -  a list of TransactionEnvelopes and its corresponding ledger it belongs to
/// * `targets` - helps in filtering out the transactions to save
/// * `filter` - logic to save the needed transaction
pub async fn listen_for_new_transactions<T, U, Filter>(
	vault_account_public_key: PublicKey,
	is_public_network: bool,
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
	let horizon_client = reqwest::Client::new();
	let mut fetcher =
		HorizonFetcher::new(horizon_client, vault_account_public_key, is_public_network);

	let mut last_cursor = 0;

	loop {
		last_cursor = fetcher
			.fetch_horizon_and_process_new_transactions(
				ledger_env_map.clone(),
				issue_map.clone(),
				memos_to_issue_ids.clone(),
				filter.clone(),
				last_cursor,
			)
			.await?;

		sleep(Duration::from_millis(POLL_INTERVAL)).await;
	}
}
