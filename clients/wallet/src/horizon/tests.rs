use std::{collections::HashMap, sync::Arc};

use crate::{
	error::Error,
	horizon::{
		responses::{HorizonClaimableBalanceResponse, TransactionResponse},
		traits::HorizonClient,
	},
};
use mockall::predicate::*;
use primitives::stellar::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::Preconditions,
	Asset, Operation, PublicKey, SecretKey, StroopAmount, Transaction, TransactionEnvelope,
};
use tokio::sync::RwLock;

use crate::types::FilterWith;

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
	let horizon_client = reqwest::Client::new();

	let source = SecretKey::from_encoding(SECRET).unwrap();
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
		"000000006f40aa9d28d31c9456fad37cfdc5be330da4c7bd166be92be7621f21075428b7";
	match horizon_client.get_claimable_balance(claimable_balance_id, false).await {
		Ok(HorizonClaimableBalanceResponse { claimable_balance }) => {
			let asset =
				std::str::from_utf8(&claimable_balance.asset).expect("should convert alright");
			assert_eq!(asset, "USDC:GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC");

			let sponsor =
				std::str::from_utf8(&claimable_balance.sponsor).expect("should convert alright");
			assert_eq!(sponsor, "GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA");
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
	match horizon_client.get_acount_transactions(public_key_encoded, true, 0, limit, false).await {
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
	let secret = SecretKey::from_encoding(SECRET).unwrap();
	let fetcher = HorizonFetcher::new(horizon_client, secret.get_public().clone(), false);

	let mut txs_iter = fetcher.fetch_transactions_iter(0).await.expect("should return a response");

	let next_page = txs_iter.next_page.clone();
	assert!(!next_page.is_empty());

	for _ in 0..txs_iter.records.len() {
		assert!(txs_iter.next().await.is_some());
	}

	// the list should be empty, as the last record was returned.
	assert_eq!(txs_iter.records.len(), 0);

	// todo: when this account's # of transactions is more than 200, add a test case for it.
}

#[tokio::test(flavor = "multi_thread")]
async fn fetch_horizon_and_process_new_transactions_success() {
	let issue_hashes = Arc::new(RwLock::new(vec![]));
	let memos_to_issue_ids = Arc::new(RwLock::new(vec![]));
	let slot_env_map = Arc::new(RwLock::new(HashMap::new()));

	let horizon_client = reqwest::Client::new();
	let secret = SecretKey::from_encoding(SECRET).unwrap();
	let mut fetcher = HorizonFetcher::new(horizon_client, secret.get_public().clone(), false);

	assert!(slot_env_map.read().await.is_empty());

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
