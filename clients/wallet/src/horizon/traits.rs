use crate::{
	error::Error,
	horizon::responses::{
		FeeStats, HorizonAccountResponse, HorizonClaimableBalanceResponse,
		HorizonTransactionsResponse, TransactionResponse,
	},
	types::PagingToken,
};
use async_trait::async_trait;
use primitives::stellar::{
	ClaimableBalanceId, PublicKey, StellarTypeToString, TransactionEnvelope,
};
use serde::de::DeserializeOwned;
use std::collections::HashMap;

#[async_trait]
pub trait HorizonClient {
	async fn get_from_url<R: DeserializeOwned>(&self, url: &str) -> Result<R, Error>;

	async fn get_account_transactions<A: StellarTypeToString<PublicKey, Error> + Send>(
		&self,
		account_id: A,
		is_public_network: bool,
		cursor: PagingToken,
		limit: u8,
		order_ascending: bool,
	) -> Result<HorizonTransactionsResponse, Error>;

	async fn get_account<A: StellarTypeToString<PublicKey, Error> + Send>(
		&self,
		account_id: A,
		is_public_network: bool,
	) -> Result<HorizonAccountResponse, Error>;

	async fn get_claimable_balance<A: StellarTypeToString<ClaimableBalanceId, Error> + Send>(
		&self,
		claimable_balance_id: A,
		is_public_network: bool,
	) -> Result<HorizonClaimableBalanceResponse, Error>;

	async fn get_fee_stats(&self, is_public_network: bool) -> Result<FeeStats, Error>;

	async fn submit_transaction(
		&self,
		transaction: TransactionEnvelope,
		is_public_network: bool,
		max_retries: u8,
		max_backoff_delay_in_secs: u16,
	) -> Result<TransactionResponse, Error>;
}

/// An important trait to check if something is empty.
pub trait IsEmptyExt {
	fn is_empty(&self) -> bool;
}

impl<K, V> IsEmptyExt for HashMap<K, V> {
	fn is_empty(&self) -> bool {
		self.is_empty()
	}
}
