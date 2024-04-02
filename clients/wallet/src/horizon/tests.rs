use std::{collections::HashMap, sync::Arc};

use crate::{
	error::Error,
	horizon::{
		responses::{HorizonClaimableBalanceResponse, TransactionResponse},
		traits::HorizonClient,
	},
	mock::secret_key_from_encoding,
	keys::get_source_secret_key_from_env,
};
#[allow(unused_imports)]
use mockall::predicate::*;

use primitives::stellar::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::Preconditions,
	Asset, Operation, PublicKey, SecretKey, StroopAmount, Transaction, TransactionEnvelope,
};
use tokio::sync::RwLock;

use crate::types::FilterWith;

use super::*;


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
	let account = horizon_client
		.get_account(source.get_encoded_public(), is_public_network)
		.await?;
	let next_sequence_number = account.sequence + 1;

	let fee_per_operation = 100;

	let mut transaction = Transaction::new(
		source.get_public().clone(),
		next_sequence_number,
		Some(fee_per_operation),
		Preconditions::PrecondNone,
		None,
	)
	.map_err(|_e| Error::BuildTransactionError("Creating new transaction failed".to_string()))?;

	let asset = Asset::native();
	let amount = StroopAmount(amount);
	transaction
		.append_operation(
			Operation::new_payment(destination, asset, amount)
				.map_err(|_e| {
					Error::BuildTransactionError("Creation of payment operation failed".to_string())
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
	let is_public_network = false;
	let horizon_client = reqwest::Client::new();
	
	let source = secret_key_from_encoding(&get_source_secret_key_from_env(is_public_network));
	// The destination is the same account as the source
	let destination = source.get_public().clone();
	let amount = 100;

	// Build simple transaction
	let tx_env = build_simple_transaction(source, destination, amount, false)
		.await
		.expect("Failed to build transaction");

	match horizon_client.submit_transaction(tx_env, false, 3, 2).await {
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
			assert_eq!(&res.account_id, public_key_encoded.as_bytes());
			assert!(res.sequence > 0);
		},
		Err(e) => {
			panic!("failed: {:?}", e);
		},
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn horizon_get_claimable_balance_success() {
	let horizon_client = reqwest::Client::new();
	let claimable_balance_id =
		"00000000fc9b9488a083f63086185807ec3f13a8de91a8c7cddc0bd06769a57ac90fb68a";
	match horizon_client.get_claimable_balance(claimable_balance_id, true).await {
		Ok(HorizonClaimableBalanceResponse { claimable_balance }) => {
			let asset =
				std::str::from_utf8(&claimable_balance.asset).expect("should convert alright");
			assert_eq!(asset, "USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN");

			let sponsor =
				std::str::from_utf8(&claimable_balance.sponsor).expect("should convert alright");
			assert_eq!(sponsor, "GCTXXQRZNQPAVBBTVZWC4QYKV3HP5HRSXZ25XDGZBYJ4PU667UGB642R");
		},
		Err(e) => {
			panic!("failed: {e:?}");
		},
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn horizon_get_transaction_success() {
	let horizon_client = reqwest::Client::new();

	let public_key_encoded = "GAYOLLLUIZE4DZMBB2ZBKGBUBZLIOYU6XFLW37GBP2VZD3ABNXCW4BVA";
	let limit = 2;
	match horizon_client
		.get_account_transactions(public_key_encoded, true, 0, limit, false)
		.await
	{
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
async fn fetch_transactions_iter_success() {
	let horizon_client = reqwest::Client::new();
	let is_public_network = false;
	let secret = secret_key_from_encoding(&get_source_secret_key_from_env(is_public_network));
	let fetcher = HorizonFetcher::new(horizon_client, secret.get_public().clone(), false);

	let mut txs_iter = fetcher.fetch_transactions_iter(0).await.expect("should return a response");

	for _ in 0..txs_iter.records.len() {
		assert!(txs_iter.next().await.is_some());
	}

	// the list should be empty, as the last record of this page was returned.
	assert_eq!(txs_iter.records.len(), 0);

	// if the next page
	let next_page = txs_iter.next_page.clone();
	if !next_page.is_empty() {
		// continue reading for transactions
		assert!(txs_iter.next().await.is_some());

		// new records can be read
		assert_ne!(txs_iter.records.len(), 0);
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn fetch_horizon_and_process_new_transactions_success() {
	let issue_hashes = Arc::new(RwLock::new(vec![]));
	let memos_to_issue_ids = Arc::new(RwLock::new(vec![]));
	let slot_env_map = Arc::new(RwLock::new(HashMap::new()));
	let is_public_network = false;

	let horizon_client = reqwest::Client::new();
	let secret = secret_key_from_encoding(&get_source_secret_key_from_env(is_public_network));
	let mut fetcher = HorizonFetcher::new(horizon_client, secret.get_public().clone(), is_public_network);

	let mut iter = fetcher.fetch_transactions_iter(0).await.expect("should return a response");
	let Some(response) = iter.next_back() else {
		assert!(false, "should have at least one transaction");
		return;
	};

	let slot = response.ledger();
	let envelope = response.to_envelope().expect("should convert to envelope");
	slot_env_map.write().await.insert(slot, envelope);

	fetcher
		.fetch_horizon_and_process_new_transactions(
			slot_env_map.clone(),
			issue_hashes.clone(),
			memos_to_issue_ids.clone(),
			MockFilter,
			0,
		)
		.await
		.expect("should fetch fine");

	assert!(!slot_env_map.read().await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn horizon_get_fee() {
	let horizon_client = reqwest::Client::new();
	assert!(horizon_client.get_fee_stats(false).await.is_ok());
	assert!(horizon_client.get_fee_stats(true).await.is_ok());
}
