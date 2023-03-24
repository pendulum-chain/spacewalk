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

use primitives::derive_shortened_request_id;

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
	async fn submit_transaction(
		&mut self,
		sequence: SequenceNumber,
		envelope: TransactionEnvelope,
		horizon_client: &Client,
	) -> Result<(TransactionResponse, TransactionEnvelope), Error> {
		let _ = self.cache.save_to_local(sequence, envelope.clone());

		let client_result = horizon_client
			.submit_transaction(envelope.clone(), self.is_public_network)
			.await
			.map(|response| (response, envelope));

		// for unit testing purposes, file won't be removed immediately.
		#[cfg(not(test))]
		let _ = self.cache.remove_transaction(sequence);

		client_result
	}
}

impl StellarWallet {
	pub fn from_secret_encoded(
		secret_key: &String,
		is_public_network: bool,
		// TODO: we need to define where the cache is stored, but maybe it shouldn't be in this
		// method.
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
	) -> (Vec<(TransactionResponse, TransactionEnvelope)>, Vec<Error>) {
		let _ = self.transaction_submission_lock.lock().await;
		let horizon_client = Client::new();

		let mut errors = vec![];
		let mut passed = vec![];
		for (env, seq) in self.cache.get_tx_envelopes() {
			match self.submit_transaction(seq, env, &horizon_client).await {
				Ok(res) => passed.push(res),
				Err(e) => {
					tracing::warn!("failed to resubmit transaction with sequence number: {}", seq);
					errors.push(e);
				},
			}
		}

		(passed, errors)
	}

	pub async fn send_payment_to_address(
		&mut self,
		destination_address: PublicKey,
		asset: Asset,
		stroop_amount: i64,
		request_id: [u8; 32],
		stroop_fee_per_operation: u32,
	) -> Result<(TransactionResponse, TransactionEnvelope), Error> {
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

		let memo_text = Memo::MemoText(
			LimitedString::new(derive_shortened_request_id(&request_id))
				.map_err(|_| Error::BuildTransactionError("Invalid hash".to_string()))?,
		);

		let mut transaction = Transaction::new(
			self.get_public_key(),
			next_sequence_number,
			Some(stroop_fee_per_operation),
			Preconditions::PrecondNone,
			Some(memo_text),
		)
		.map_err(|_e| {
			Error::BuildTransactionError("Creating new transaction failed".to_string())
		})?;

		let amount = StroopAmount(stroop_amount);
		transaction
			.append_operation(
				Operation::new_payment(destination_address, asset, amount)
					.map_err(|_e| {
						Error::BuildTransactionError(
							"Creation of payment operation failed".to_string(),
						)
					})?
					.set_source_account(self.get_public_key())
					.map_err(|_e| {
						Error::BuildTransactionError("Setting source account failed".to_string())
					})?,
			)
			.map_err(|_e| {
				Error::BuildTransactionError("Appending payment operation failed".to_string())
			})?;

		let mut envelope = transaction.into_transaction_envelope();
		let network: &Network =
			if self.is_public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

		envelope
			.sign(network, vec![&self.get_secret_key()])
			.map_err(|_e| Error::SignEnvelopeError)?;

		self.submit_transaction(next_sequence_number, envelope, &horizon_client).await
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
		if !self.cache.remove_all() {
			tracing::warn!("the cache was not removed. Please delete manually.");
		}
	}
}

#[cfg(test)]
mod test {
	use primitives::TransactionEnvelopeExt;
	use serial_test::serial;
	use std::sync::Arc;
	use substrate_stellar_sdk::{PublicKey, XdrCodec};
	use tokio::sync::RwLock;

	use crate::StellarWallet;

	const STELLAR_VAULT_SECRET_KEY: &str =
		"SCV7RZN5XYYMMVSWYCR4XUMB76FFMKKKNHP63UTZQKVM4STWSCIRLWFJ";
	const IS_PUBLIC_NETWORK: bool = false;

	fn wallet() -> Arc<RwLock<StellarWallet>> {
		Arc::new(RwLock::new(
			StellarWallet::from_secret_encoded(
				&STELLAR_VAULT_SECRET_KEY.to_string(),
				IS_PUBLIC_NETWORK,
				"resources/test_2".to_string(),
			)
			.unwrap(),
		))
	}

	#[tokio::test]
	#[serial]
	async fn test_locking_submission() {
		let wallet_clone = wallet().clone();
		let first_job = tokio::spawn(async move {
			let destination = PublicKey::from_encoding(
				"GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA",
			)
			.unwrap();
			let asset = substrate_stellar_sdk::Asset::native();
			let amount = 100;
			let request_id = [0u8; 32];

			let result = wallet_clone
				.write()
				.await
				.send_payment_to_address(destination, asset, amount, request_id, 100)
				.await;

			assert!(result.is_ok());
			let (transaction_response, env) = result.unwrap();
			assert!(!transaction_response.hash.to_vec().is_empty());
			assert!(transaction_response.ledger() > 0);

			// remove the files to not pollute the project.
			let seq = env.sequence_number().expect("to return a sequence number");
			assert!(wallet().read().await.cache.remove_transaction(seq));
		});

		let wallet_clone2 = wallet().clone();
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

			assert!(result.is_ok());
			let (transaction_response, env) = result.unwrap();
			assert!(!transaction_response.hash.to_vec().is_empty());
			assert!(transaction_response.ledger() > 0);

			// remove the files to not pollute the project.
			let seq = env.sequence_number().expect("to return a sequence number");
			assert!(wallet().read().await.cache.remove_transaction(seq));
		});

		let _ = tokio::join!(first_job, second_job);
	}

	#[tokio::test]
	#[serial]
	async fn sending_payment_works() {
		let destination =
			PublicKey::from_encoding("GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA")
				.unwrap();
		let asset = substrate_stellar_sdk::Asset::native();
		let amount = 100;
		let request_id = [0u8; 32];

		let result = wallet()
			.write()
			.await
			.send_payment_to_address(destination, asset, amount, request_id, 100)
			.await;

		assert!(result.is_ok());
		let (transaction_response, env) = result.unwrap();
		assert!(!transaction_response.hash.to_vec().is_empty());
		assert!(transaction_response.ledger() > 0);

		// remove the file to not pollute the project.
		let seq = env.sequence_number().expect("to return a sequence number");
		assert!(wallet().read().await.cache.remove_transaction(seq));
	}

	#[tokio::test]
	#[serial]
	async fn sending_correct_payment_after_incorrect_payment_works() {
		let wallet = wallet().clone();
		let mut wallet = wallet.write().await;

		let destination =
			PublicKey::from_encoding("GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA")
				.unwrap();
		let asset = substrate_stellar_sdk::Asset::native();
		let amount = 1000;
		let request_id = [0u8; 32];
		let correct_amount_that_should_not_fail = 100;
		let incorrect_amount_that_should_fail = 0;

		let (_, env) = wallet
			.send_payment_to_address(
				destination.clone(),
				asset.clone(),
				amount,
				request_id,
				correct_amount_that_should_not_fail,
			)
			.await
			.expect("should be ok");

		// remove the file to not pollute the project.
		let seq = env.sequence_number().expect("to return a sequence number");
		assert!(wallet.cache.remove_transaction(seq));

		let err_insufficient_fee = wallet
			.send_payment_to_address(
				destination.clone(),
				asset.clone(),
				amount,
				request_id,
				incorrect_amount_that_should_fail,
			)
			.await;

		assert!(!err_insufficient_fee.is_ok());

		let (_, env) = wallet
			.send_payment_to_address(
				destination.clone(),
				asset.clone(),
				amount,
				request_id,
				correct_amount_that_should_not_fail,
			)
			.await
			.expect("this should already work");

		// remove the files to not pollute the project.
		let seq = env.sequence_number().expect("to return a sequence number");
		assert!(wallet.cache.remove_transaction(seq));
	}

	#[tokio::test]
	#[serial]
	async fn resubmit_transactions_works() {
		// there are 2 acceptable files in the directory, but only 1 will pass; the other should
		// fail.
		let (passed, failed) = wallet().write().await.resubmit_transactions_from_cache().await;
		assert_eq!(passed.len(), 1);
		let (resp, env) = &passed[0];
		assert_eq!(&resp.envelope_xdr, &env.to_base64_xdr());

		assert_eq!(failed.len(), 1);
	}
}
