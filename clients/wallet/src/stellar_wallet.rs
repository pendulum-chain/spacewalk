use reqwest::Client;
use std::{fmt::Formatter, sync::Arc};

use substrate_stellar_sdk::{
	compound_types::LimitedString,
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::{Preconditions, SequenceNumber},
	Asset, Memo, Operation, PublicKey, SecretKey, StroopAmount, Transaction, TransactionEnvelope,
};
use tokio::sync::Mutex;

use crate::{
	cache::TxEnvelopeStorage,
	error::Error,
	horizon::{HorizonClient, PagingToken, TransactionResponse},
	types::StellarPublicKeyRaw,
};

use primitives::{derive_shortened_request_id, TransactionEnvelopeExt};

#[derive(Clone)]
pub struct StellarWallet {
	secret_key: SecretKey,
	is_public_network: bool,
	/// Used to make sure that only one transaction is submitted at a time,
	/// so that the transaction is not rejected due to an outdated sequence number.
	/// Releasing the lock ensures the sequence number of the account
	/// has been increased on the network.
	transaction_submission_lock: Arc<Mutex<()>>,
	/// Used for caching Stellar transactions before they get submitted.
	cache: TxEnvelopeStorage,
}

impl StellarWallet {
	/// Returns a TransactionResponse after submitting transaction envelope to Stellar,
	/// Else an Error.
	async fn submit_transaction(
		&mut self,
		envelope: TransactionEnvelope,
		horizon_client: &Client,
	) -> Result<TransactionResponse, Error> {
		let sequence = &envelope
			.sequence_number()
			.ok_or(Error::UnknownSequenceNumber(envelope.clone()))?;

		let submission_result =
			horizon_client.submit_transaction(envelope, self.is_public_network).await;

		let _ = self.cache.remove_transaction(*sequence);

		submission_result
	}
}

#[doc(hidden)]
/// Creates and returns a Transaction
fn create_transaction(
	destination_address: PublicKey,
	asset: Asset,
	stroop_amount: i64,
	request_id: [u8; 32],
	stroop_fee_per_operation: u32,
	public_key: PublicKey,
	next_sequence_number: SequenceNumber,
) -> Result<Transaction, Error> {
	let memo_text = Memo::MemoText(
		LimitedString::new(derive_shortened_request_id(&request_id))
			.map_err(|_| Error::BuildTransactionError("Invalid hash".to_string()))?,
	);

	let mut transaction = Transaction::new(
		public_key.clone(),
		next_sequence_number,
		Some(stroop_fee_per_operation),
		Preconditions::PrecondNone,
		Some(memo_text),
	)
	.map_err(|_e| Error::BuildTransactionError("Creating new transaction failed".to_string()))?;

	let amount = StroopAmount(stroop_amount);
	transaction
		.append_operation(
			Operation::new_payment(destination_address, asset, amount)
				.map_err(|_e| {
					Error::BuildTransactionError("Creation of payment operation failed".to_string())
				})?
				.set_source_account(public_key)
				.map_err(|_e| {
					Error::BuildTransactionError("Setting source account failed".to_string())
				})?,
		)
		.map_err(|_e| {
			Error::BuildTransactionError("Appending payment operation failed".to_string())
		})?;

	Ok(transaction)
}

impl StellarWallet {
	pub fn from_secret_encoded(
		secret_key: &String,
		is_public_network: bool,
	) -> Result<Self, Error> {
		Self::from_secret_encoded_with_cache(secret_key, is_public_network, "./".to_string())
	}

	/// creates a wallet based on the secret key,
	/// and can specify the path where the cache will be saved.
	pub fn from_secret_encoded_with_cache(
		secret_key: &String,
		is_public_network: bool,
		cache_path: String,
	) -> Result<Self, Error> {
		let secret_key =
			SecretKey::from_encoding(secret_key).map_err(|_| Error::InvalidSecretKey)?;

		let pub_key = secret_key.get_public().to_encoding();
		let pub_key = std::str::from_utf8(&pub_key).map_err(|_| Error::InvalidSecretKey)?;

		let cache = TxEnvelopeStorage::new(cache_path, pub_key, is_public_network);

		let wallet = StellarWallet {
			secret_key,
			is_public_network,
			transaction_submission_lock: Arc::new(Mutex::new(())),
			cache,
		};

		Ok(wallet)
	}

	pub fn get_public_key_raw(&self) -> StellarPublicKeyRaw {
		self.secret_key.get_public().clone().into_binary()
	}

	pub fn get_public_key(&self) -> PublicKey {
		self.secret_key.get_public().clone()
	}

	pub fn get_secret_key(&self) -> SecretKey {
		self.secret_key.clone()
	}

	pub fn is_public_network(&self) -> bool {
		self.is_public_network
	}

	pub async fn get_latest_transactions(
		&self,
		cursor: PagingToken,
		limit: i64,
		order_ascending: bool,
	) -> Result<Vec<TransactionResponse>, Error> {
		let horizon_client = reqwest::Client::new();

		let public_key_encoded = self.get_public_key().to_encoding();
		let account_id = std::str::from_utf8(&public_key_encoded).map_err(Error::Utf8Error)?;

		let transactions_response = horizon_client
			.get_transactions(account_id, self.is_public_network, cursor, limit, order_ascending)
			.await?;

		let transactions = transactions_response._embedded.records;

		Ok(transactions)
	}

	/// Submits transactions found in the wallet's cache to Stellar.
	/// Returns a tuple: (list of txs successfully submitted, list of errors of txs not resubmitted)
	pub async fn resubmit_transactions_from_cache(
		&mut self,
	) -> (Vec<TransactionResponse>, Vec<Error>) {
		let _ = self.transaction_submission_lock.lock().await;
		let horizon_client = Client::new();

		let mut passed = vec![];

		let (envs, mut errors) = match self.cache.get_tx_envelopes() {
			Ok(x) => x,
			Err(errors) => return (passed, errors),
		};

		for env in envs.into_iter() {
			match self.submit_transaction(env, &horizon_client).await {
				Ok(response) => passed.push(response),
				Err(e) => errors.push(e),
			}
		}

		(passed, errors)
	}

	fn create_envelope(
		&self,
		destination_address: PublicKey,
		asset: Asset,
		stroop_amount: i64,
		request_id: [u8; 32],
		stroop_fee_per_operation: u32,
		next_sequence_number: SequenceNumber,
	) -> Result<TransactionEnvelope, Error> {
		let transaction = create_transaction(
			destination_address,
			asset,
			stroop_amount,
			request_id,
			stroop_fee_per_operation,
			self.get_public_key(),
			next_sequence_number,
		)?;

		let mut envelope = transaction.into_transaction_envelope();
		let network: &Network =
			if self.is_public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

		envelope
			.sign(network, vec![&self.get_secret_key()])
			.map_err(|_e| Error::SignEnvelopeError)?;

		Ok(envelope)
	}

	pub async fn send_payment_to_address(
		&mut self,
		destination_address: PublicKey,
		asset: Asset,
		stroop_amount: i64,
		request_id: [u8; 32],
		stroop_fee_per_operation: u32,
	) -> Result<TransactionResponse, Error> {
		let _ = self.transaction_submission_lock.lock().await;
		let horizon_client = reqwest::Client::new();

		let public_key_encoded = self.get_public_key().to_encoding();
		let account_id_string =
			std::str::from_utf8(&public_key_encoded).map_err(Error::Utf8Error)?;
		let account = horizon_client.get_account(account_id_string, self.is_public_network).await?;
		let next_sequence_number = account.sequence + 1;

		tracing::info!(
			"Next sequence number: {} for account: {:?}",
			next_sequence_number,
			account.account_id
		);

		let envelope = self.create_envelope(
			destination_address,
			asset,
			stroop_amount,
			request_id,
			stroop_fee_per_operation,
			next_sequence_number,
		)?;

		let _ = self.cache.save_to_local(envelope.clone())?;
		self.submit_transaction(envelope, &horizon_client).await
	}
}

impl std::fmt::Debug for StellarWallet {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let public_key_encoded = self.get_public_key().to_encoding();
		let account_id_string =
			std::str::from_utf8(&public_key_encoded).map_err(|_e| std::fmt::Error)?;
		write!(
			f,
			"StellarWallet [public key: {}, public network: {}]",
			account_id_string, self.is_public_network
		)
	}
}

#[cfg(feature = "testing-utils")]
impl Drop for StellarWallet {
	fn drop(&mut self) {
		tracing::debug!("deleting cache...");
		if !self.cache.remove_dir() {
			tracing::warn!("the cache was not removed. Please delete manually.");
		}
	}
}

#[cfg(test)]
mod test {
	use crate::error::Error;
	use primitives::TransactionEnvelopeExt;
	use serial_test::serial;
	use std::sync::Arc;
	use substrate_stellar_sdk::{PublicKey, TransactionEnvelope, XdrCodec};
	use tokio::sync::RwLock;

	use crate::StellarWallet;

	const STELLAR_VAULT_SECRET_KEY: &str =
		"SCV7RZN5XYYMMVSWYCR4XUMB76FFMKKKNHP63UTZQKVM4STWSCIRLWFJ";
	const IS_PUBLIC_NETWORK: bool = false;

	fn wallet(storage: &str) -> Arc<RwLock<StellarWallet>> {
		Arc::new(RwLock::new(
			StellarWallet::from_secret_encoded_with_cache(
				&STELLAR_VAULT_SECRET_KEY.to_string(),
				IS_PUBLIC_NETWORK,
				storage.to_string(),
			)
			.unwrap(),
		))
	}

	#[tokio::test]
	#[serial]
	async fn test_locking_submission() {
		let wallet = wallet("resources/test_locking_submission").clone();
		let wallet_clone = wallet.clone();

		let first_job = tokio::spawn(async move {
			let destination = PublicKey::from_encoding(
				"GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA",
			)
			.unwrap();
			let asset = substrate_stellar_sdk::Asset::native();
			let amount = 100;
			let request_id = [0u8; 32];

			let response = wallet_clone
				.write()
				.await
				.send_payment_to_address(destination, asset, amount, request_id, 100)
				.await
				.expect("it should return a success");

			assert!(!response.hash.to_vec().is_empty());
			assert!(response.ledger() > 0);
		});

		let wallet_clone2 = wallet.clone();
		let second_job = tokio::spawn(async move {
			let destination = PublicKey::from_encoding(
				"GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA",
			)
			.unwrap();
			let asset = substrate_stellar_sdk::Asset::native();
			let amount = 50;
			let request_id = [1u8; 32];

			let result = wallet_clone2
				.write()
				.await
				.send_payment_to_address(destination, asset, amount, request_id, 100)
				.await;

			let transaction_response = result.expect("should return a transaction response");
			assert!(!transaction_response.hash.to_vec().is_empty());
			assert!(transaction_response.ledger() > 0);
		});

		let _ = tokio::join!(first_job, second_job);

		assert!(wallet.write().await.cache.remove_dir());
	}

	#[tokio::test]
	#[serial]
	async fn sending_payment_works() {
		let wallet = wallet("resources/sending_payment_works");
		let destination =
			PublicKey::from_encoding("GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA")
				.unwrap();
		let asset = substrate_stellar_sdk::Asset::native();
		let amount = 100;
		let request_id = [0u8; 32];

		let result = wallet
			.write()
			.await
			.send_payment_to_address(destination, asset, amount, request_id, 100)
			.await;

		assert!(result.is_ok());
		let transaction_response = result.unwrap();
		assert!(!transaction_response.hash.to_vec().is_empty());
		assert!(transaction_response.ledger() > 0);
		assert!(wallet.write().await.cache.remove_dir());
	}

	#[tokio::test]
	#[serial]
	async fn sending_correct_payment_after_incorrect_payment_works() {
		let wallet =
			wallet("resources/sending_correct_payment_after_incorrect_payment_works").clone();
		let mut wallet = wallet.write().await;

		// let's cleanup, just to make sure.
		wallet.cache.remove_all_transactions();

		let destination =
			PublicKey::from_encoding("GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA")
				.unwrap();
		let asset = substrate_stellar_sdk::Asset::native();
		let amount = 1000;
		let request_id = [0u8; 32];
		let correct_amount_that_should_not_fail = 100;
		let incorrect_amount_that_should_fail = 0;

		let response = wallet
			.send_payment_to_address(
				destination.clone(),
				asset.clone(),
				amount,
				request_id,
				correct_amount_that_should_not_fail,
			)
			.await;

		assert!(response.is_ok());

		let err_insufficient_fee = wallet
			.send_payment_to_address(
				destination.clone(),
				asset.clone(),
				amount,
				request_id,
				incorrect_amount_that_should_fail,
			)
			.await;

		assert!(err_insufficient_fee.is_err());
		match err_insufficient_fee.unwrap_err() {
			Error::HorizonSubmissionError { title: _, status: _, reason, envelope_xdr: _ } => {
				assert_eq!(reason, "tx_insufficient_fee");
			},
			_ => assert!(false),
		}

		let tx_response = wallet
			.send_payment_to_address(
				destination.clone(),
				asset.clone(),
				amount,
				request_id,
				correct_amount_that_should_not_fail,
			)
			.await;

		assert!(tx_response.is_ok());

		assert!(wallet.cache.remove_dir());
	}

	#[tokio::test]
	#[serial]
	async fn resubmit_transactions_works() {
		let wallet = wallet("resources/resubmit_transactions_works").clone();
		let mut wallet = wallet.write().await;

		// let's send a successful transaction first
		let destination =
			PublicKey::from_encoding("GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA")
				.unwrap();
		let asset = substrate_stellar_sdk::Asset::native();
		let amount = 1001;
		let stroop_fee = 100;
		let request_id = [0u8; 32];

		let response = wallet
			.send_payment_to_address(
				destination.clone(),
				asset.clone(),
				amount,
				request_id,
				stroop_fee,
			)
			.await
			.expect("should be ok");

		// get the sequence number of the previous one.
		let env =
			TransactionEnvelope::from_base64_xdr(response.envelope_xdr).expect("should convert ok");
		let seq_number = env.sequence_number().expect("should return sequence number");

		// creating a `tx_bad_seq` envelope.
		let request_id = [1u8; 32];
		let bad_envelope = wallet
			.create_envelope(
				destination.clone(),
				asset.clone(),
				amount,
				request_id,
				stroop_fee,
				seq_number,
			)
			.expect("should return an envelope");

		// let's save this in storage
		let _ = wallet.cache.save_to_local(bad_envelope.clone()).expect("should save.");

		// create a successful transaction
		let request_id = [1u8; 32];
		let good_envelope = wallet
			.create_envelope(destination, asset, amount, request_id, stroop_fee, seq_number + 1)
			.expect("should return an envelope");

		// let's save this in storage
		let _ = wallet.cache.save_to_local(good_envelope.clone()).expect("should save");

		// 1 should pass, and 1 should fail.
		let (passed, failed) = wallet.resubmit_transactions_from_cache().await;
		assert_eq!(passed.len(), 1);
		assert_eq!(failed.len(), 1);

		assert_eq!(passed[0].envelope_xdr, good_envelope.to_base64_xdr());

		match &failed[0] {
			Error::HorizonSubmissionError { title: _, status: _, reason, envelope_xdr: _ } => {
				assert_eq!(reason, "tx_bad_seq");
			},
			_ => assert!(false),
		}

		assert!(wallet.cache.remove_dir());
	}
}
