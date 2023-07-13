use crate::{
	error::Error,
	horizon::responses::{
		HorizonAccountResponse, HorizonClaimableBalanceResponse, HorizonTransactionsResponse,
		TransactionResponse,
	},
	types::PagingToken,
};
use async_trait::async_trait;
use primitives::StellarTypeToString;
use serde::de::DeserializeOwned;
use substrate_stellar_sdk::{ClaimableBalanceId, PublicKey, TransactionEnvelope};

#[async_trait]
pub trait HorizonClient {
	async fn get_from_url<R: DeserializeOwned>(&self, url: &str) -> Result<R, Error>;

	async fn get_transactions<A: StellarTypeToString<PublicKey, Error> + Send>(
		&self,
		account_id: A,
		is_public_network: bool,
		cursor: PagingToken,
		limit: i64,
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

	async fn submit_transaction(
		&self,
		transaction: TransactionEnvelope,
		is_public_network: bool,
		max_retries: u8,
		max_backoff_delay_in_secs: u16,
	) -> Result<TransactionResponse, Error>;
}
