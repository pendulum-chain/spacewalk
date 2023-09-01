use reqwest::Client;
use std::{fmt::Formatter, sync::Arc};

use primitives::stellar::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::SequenceNumber,
	Asset as StellarAsset, Operation, PublicKey, SecretKey, TransactionEnvelope,
};
use tokio::sync::{oneshot, Mutex};

use crate::{
	cache::WalletStateStorage,
	error::Error,
	horizon::{
		responses::{HorizonBalance, TransactionResponse},
		HorizonClient,
	},
};

use crate::{
	error::CacheErrorKind,
	horizon::{responses::TransactionsResponseIter, DEFAULT_PAGE_SIZE},
	operations::{
		create_basic_spacewalk_stellar_transaction, create_payment_operation, AppendExt,
		RedeemOperationsExt,
	},
	types::PagingToken,
};
use primitives::{
	StellarPublicKeyRaw, StellarStroops, StellarTypeToString, TransactionEnvelopeExt,
};

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
	/// Also used for caching the latest cursor to page through Stellar transactions in horizon
	cache: WalletStateStorage,

	/// maximum retry attempts for submitting a transaction before switching to a fallback url
	max_retry_attempts_before_fallback: u8,

	/// the waiting time (in seconds) for retrying.
	max_backoff_delay: u16,

	/// a client to connect to Horizon
	client: reqwest::Client,
}

impl StellarWallet {
	/// if the user doesn't define the maximum number of retry attempts for 500 internal server
	/// error, this will be the default.
	const DEFAULT_MAX_RETRY_ATTEMPTS_BEFORE_FALLBACK: u8 = 3;

	const DEFAULT_MAX_BACKOFF_DELAY_IN_SECS: u16 = 600;

	/// Returns a TransactionResponse after submitting transaction envelope to Stellar,
	/// Else an Error.
	async fn submit_transaction(
		&self,
		envelope: TransactionEnvelope,
	) -> Result<TransactionResponse, Error> {
		let sequence = &envelope.sequence_number().ok_or(Error::cache_error_with_env(
			CacheErrorKind::UnknownSequenceNumber,
			envelope.clone(),
		))?;

		let submission_result = self
			.client
			.submit_transaction(
				envelope.clone(),
				self.is_public_network,
				self.max_retry_attempts_before_fallback,
				self.max_backoff_delay,
			)
			.await;

		let _ = self.cache.remove_tx_envelope(*sequence);

		submission_result
	}
}

impl StellarWallet {
	pub fn from_secret_encoded(secret_key: &str, is_public_network: bool) -> Result<Self, Error> {
		Self::from_secret_encoded_with_cache(secret_key, is_public_network, "./".to_string())
	}

	/// creates a wallet based on the secret key,
	/// and can specify the path where the cache will be saved.
	pub fn from_secret_encoded_with_cache(
		secret_key: &str,
		is_public_network: bool,
		cache_path: String,
	) -> Result<Self, Error> {
		let secret_key =
			SecretKey::from_encoding(secret_key).map_err(|_| Error::InvalidSecretKey)?;

		Self::from_secret_key_with_cache(secret_key, is_public_network, cache_path)
	}

	pub fn from_secret_key(secret_key: SecretKey, is_public_network: bool) -> Result<Self, Error> {
		Self::from_secret_key_with_cache(secret_key, is_public_network, "./".to_string())
	}

	pub fn from_secret_key_with_cache(
		secret_key: SecretKey,
		is_public_network: bool,
		cache_path: String,
	) -> Result<Self, Error> {
		let pub_key = secret_key.get_public().as_encoded_string().map_err(|e: Error| {
			tracing::error!(
				"Failed to create StellarWallet due to invalid encoding public key: {e:?}"
			);
			Error::InvalidSecretKey
		})?;

		let cache = WalletStateStorage::new(cache_path, &pub_key, is_public_network);

		Ok(StellarWallet {
			secret_key,
			is_public_network,
			transaction_submission_lock: Arc::new(Mutex::new(())),
			cache,
			max_retry_attempts_before_fallback: Self::DEFAULT_MAX_RETRY_ATTEMPTS_BEFORE_FALLBACK,
			max_backoff_delay: Self::DEFAULT_MAX_BACKOFF_DELAY_IN_SECS,
			client: reqwest::Client::new(),
		})
	}

	pub fn with_max_retry_attempts_before_fallback(mut self, max_retries: u8) -> Self {
		self.max_retry_attempts_before_fallback = max_retries;

		self
	}

	pub fn with_max_backoff_delay(mut self, max_backoff_delay_in_secs: u16) -> Self {
		// a number more than the default max would be too large
		if max_backoff_delay_in_secs < Self::DEFAULT_MAX_BACKOFF_DELAY_IN_SECS {
			self.max_backoff_delay = max_backoff_delay_in_secs;
		}

		self
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

	pub fn get_last_cursor(&self) -> PagingToken {
		self.cache.get_last_cursor()
	}

	pub fn save_cursor(&self, paging_token: PagingToken) -> Result<(), Error> {
		self.cache.save_cursor(paging_token)
	}

	/// Returns an iter for all transactions.
	/// This method is looking BACKWARDS, so the transactions are in DESCENDING order:
	/// starting from the LATEST ones, at the time of the call.
	pub async fn get_all_transactions_iter(
		&self,
	) -> Result<TransactionsResponseIter<Client>, Error> {
		let horizon_client = Client::new();

		let transactions_response = horizon_client
			.get_account_transactions(
				self.get_public_key(),
				self.is_public_network,
				0,
				DEFAULT_PAGE_SIZE,
				false,
			)
			.await?;

		let next_page = transactions_response.next_page();
		let records = transactions_response.records();

		Ok(TransactionsResponseIter { records, next_page, client: horizon_client })
	}

	/// Returns the balances of this wallet's Stellar account
	pub async fn get_balances(&self) -> Result<Vec<HorizonBalance>, Error> {
		let account =
			self.client.get_account(self.get_public_key(), self.is_public_network).await?;
		Ok(account.balances)
	}
}

// send/submit functions of StellarWallet
impl StellarWallet {
	/// Submits transactions found in the wallet's cache to Stellar.
	/// Returns a list of oneshot receivers to send back the result of resubmission.
	pub async fn resubmit_transactions_from_cache(
		&self,
	) -> Vec<oneshot::Receiver<Result<TransactionResponse, Error>>> {
		let _ = self.transaction_submission_lock.lock().await;

		// Iterates over all errors and creates channels that are used to send errors back to the
		// caller of this function.
		let mut error_receivers = vec![];

		let mut collect_errors = |errors: Vec<Error>| {
			for error in errors {
				let (sender, receiver) = oneshot::channel();
				error_receivers.push(receiver);

				if let Err(e) = sender.send(Err(error)) {
					tracing::error!(
						"Failed to send error to list during transaction resubmission: {e:?}"
					);
				}
			}
		};

		let envs = match self.cache.get_tx_envelopes() {
			Ok((envs, errors)) => {
				collect_errors(errors);
				envs
			},
			Err(errors) => {
				collect_errors(errors);
				return error_receivers
			},
		};

		let me = Arc::new(self.clone());
		for env in envs.into_iter() {
			let me_clone = Arc::clone(&me);

			let (sender, receiver) = oneshot::channel();
			error_receivers.push(receiver);

			tokio::spawn(async move {
				if let Err(e) = sender.send(me_clone.submit_transaction(env).await) {
					tracing::error!(
						"Failed to send message during transaction resubmission: {e:?}"
					);
				};
			});
		}

		error_receivers
	}

	fn create_envelope(
		&self,
		request_id: [u8; 32],
		stroop_fee_per_operation: u32,
		next_sequence_number: SequenceNumber,
		operations: Vec<Operation>,
	) -> Result<TransactionEnvelope, Error> {
		let public_key = self.get_public_key();

		// create the transaction
		let mut transaction = create_basic_spacewalk_stellar_transaction(
			request_id,
			stroop_fee_per_operation,
			public_key,
			next_sequence_number,
		)?;

		// add operations
		transaction.append_multiple(operations)?;

		// convert to envelope
		let mut envelope = transaction.into_transaction_envelope();
		let network: &Network =
			if self.is_public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

		envelope
			.sign(network, vec![&self.get_secret_key()])
			.map_err(|_e| Error::SignEnvelopeError)?;

		Ok(envelope)
	}

	/// Sends a 'Payment' transaction.
	///
	/// # Arguments
	/// * `destination_address` - receiver of the payment
	/// * `asset` - Stellar Asset type of the payment
	/// * `stroop_amount` - Amount of the payment
	/// * `request_id` - information to be added in the tx's memo
	/// * `stroop_fee_per_operation` - base fee to pay for the payment operation
	/// * `is_payment_for_redeem_request` - true if the operation is for redeem request
	pub async fn send_payment_to_address(
		&mut self,
		destination_address: PublicKey,
		asset: StellarAsset,
		stroop_amount: StellarStroops,
		request_id: [u8; 32],
		stroop_fee_per_operation: u32,
		is_payment_for_redeem_request: bool,
	) -> Result<TransactionResponse, Error> {
		// user must not send to self
		if self.secret_key.get_public() == &destination_address {
			return Err(Error::SelfPaymentError)
		}

		// create payment operation
		let payment_op = if is_payment_for_redeem_request {
			self.client
				.create_payment_op_for_redeem_request(
					self.get_public_key(),
					destination_address,
					self.is_public_network,
					asset,
					stroop_amount,
				)
				.await?
		} else {
			create_payment_operation(
				destination_address,
				asset,
				stroop_amount,
				self.get_public_key(),
			)?
		};

		self.send_to_address(request_id, stroop_fee_per_operation, vec![payment_op])
			.await
	}

	async fn send_to_address(
		&mut self,
		request_id: [u8; 32],
		stroop_fee_per_operation: u32,
		operations: Vec<Operation>,
	) -> Result<TransactionResponse, Error> {
		let _ = self.transaction_submission_lock.lock().await;

		let account =
			self.client.get_account(self.get_public_key(), self.is_public_network).await?;
		let next_sequence_number = account.sequence + 1;

		tracing::trace!(
			"submitting transaction: Next sequence number: {} for account: {:?}",
			next_sequence_number,
			account.account_id
		);

		let envelope = self.create_envelope(
			request_id,
			stroop_fee_per_operation,
			next_sequence_number,
			operations,
		)?;

		let _ = self.cache.save_tx_envelope(envelope.clone())?;

		self.submit_transaction(envelope).await
	}
}

impl std::fmt::Debug for StellarWallet {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let account_id_string = self
			.secret_key
			.get_public()
			.as_encoded_string()
			.map_err(|_: Error| std::fmt::Error)?;

		write!(
			f,
			"StellarWallet [public key: {}, public network: {}]",
			account_id_string, self.is_public_network
		)
	}
}

#[cfg(test)]
mod test {
	use crate::{
		error::Error,
		horizon::{responses::HorizonClaimableBalanceResponse, HorizonClient},
		operations::{
			create_payment_operation, redeem_request_tests::create_account_merge_operation,
		},
		TransactionResponse,
	};
	use primitives::{
		stellar::{
			types::{
				CreateAccountResult, CreateClaimableBalanceResult, OperationResult,
				OperationResultTr, SequenceNumber,
			},
			Asset as StellarAsset, PublicKey, SecretKey, TransactionEnvelope, XdrCodec,
		},
		StellarStroops, TransactionEnvelopeExt,
	};
	use serial_test::serial;
	use std::sync::Arc;
	use tokio::sync::RwLock;

	use crate::{test_helper::default_usdc_asset, StellarWallet};

	const DEFAULT_DEST_PUBLIC_KEY: &str =
		"GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";
	const STELLAR_VAULT_SECRET_KEY: &str =
		"SCV7RZN5XYYMMVSWYCR4XUMB76FFMKKKNHP63UTZQKVM4STWSCIRLWFJ";
	const IS_PUBLIC_NETWORK: bool = false;

	const DEFAULT_STROOP_FEE_PER_OPERATION: u32 = 100;

	impl StellarWallet {
		async fn is_account_exist(&self) -> bool {
			self.client
				.get_account(self.get_public_key(), self.is_public_network)
				.await
				.is_ok()
		}

		/// merges the wallet's account with the specified destination.
		/// Exercise prudence when using this method, as it automatically removes the source account
		/// once operation is successful.
		async fn merge_account(
			&mut self,
			destination_address: PublicKey,
		) -> Result<TransactionResponse, Error> {
			let account_merge_op = create_account_merge_operation(
				destination_address,
				self.secret_key.get_public().clone(),
			)?;

			self.send_to_address(
				[9u8; 32],
				DEFAULT_STROOP_FEE_PER_OPERATION,
				vec![account_merge_op],
			)
			.await
		}

		fn create_payment_envelope(
			&self,
			destination_address: PublicKey,
			asset: StellarAsset,
			stroop_amount: StellarStroops,
			request_id: [u8; 32],
			stroop_fee_per_operation: u32,
			next_sequence_number: SequenceNumber,
		) -> Result<TransactionEnvelope, Error> {
			let public_key = self.get_public_key();
			// create payment operation
			let payment_op = create_payment_operation(
				destination_address,
				asset,
				stroop_amount,
				public_key.clone(),
			)?;

			self.create_envelope(
				request_id,
				stroop_fee_per_operation,
				next_sequence_number,
				vec![payment_op],
			)
		}
	}

	fn wallet_with_storage(storage: &str) -> Arc<RwLock<StellarWallet>> {
		wallet_with_secret_key_for_storage(storage, STELLAR_VAULT_SECRET_KEY)
	}

	fn wallet_with_secret_key_for_storage(
		storage: &str,
		secret_key: &str,
	) -> Arc<RwLock<StellarWallet>> {
		Arc::new(RwLock::new(
			StellarWallet::from_secret_encoded_with_cache(
				secret_key,
				IS_PUBLIC_NETWORK,
				storage.to_string(),
			)
			.unwrap(),
		))
	}

	fn default_destination() -> PublicKey {
		PublicKey::from_encoding(DEFAULT_DEST_PUBLIC_KEY).expect("Should return a public key")
	}

	#[test]
	fn test_add_backoff_delay() {
		let wallet = StellarWallet::from_secret_encoded_with_cache(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			IS_PUBLIC_NETWORK,
			"resources/test_add_backoff_delay".to_owned(),
		)
		.unwrap();

		assert_eq!(wallet.max_backoff_delay, StellarWallet::DEFAULT_MAX_BACKOFF_DELAY_IN_SECS);

		// too big backoff delay
		let expected_max_backoff_delay = 800;
		let new_wallet = wallet.with_max_backoff_delay(expected_max_backoff_delay);
		assert_ne!(new_wallet.max_backoff_delay, expected_max_backoff_delay);

		let expected_max_backoff_delay = 300;
		let new_wallet = new_wallet.with_max_backoff_delay(expected_max_backoff_delay);
		assert_eq!(new_wallet.max_backoff_delay, expected_max_backoff_delay);

		new_wallet.cache.remove_dir();
	}

	#[test]
	fn test_add_retry_attempt() {
		let wallet = StellarWallet::from_secret_encoded_with_cache(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			IS_PUBLIC_NETWORK,
			"resources/test_add_retry_attempt".to_owned(),
		)
		.unwrap();

		assert_eq!(
			wallet.max_retry_attempts_before_fallback,
			StellarWallet::DEFAULT_MAX_RETRY_ATTEMPTS_BEFORE_FALLBACK
		);

		let expected_max_retries = 5;
		let new_wallet = wallet.with_max_retry_attempts_before_fallback(expected_max_retries);
		assert_eq!(new_wallet.max_retry_attempts_before_fallback, expected_max_retries);

		new_wallet.cache.remove_dir();
	}

	#[tokio::test]
	#[serial]
	async fn test_locking_submission() {
		let wallet = wallet_with_storage("resources/test_locking_submission").clone();
		let wallet_clone = wallet.clone();

		let first_job = tokio::spawn(async move {
			let asset = StellarAsset::native();
			let amount = 100;
			let request_id = [0u8; 32];

			let response = wallet_clone
				.write()
				.await
				.send_payment_to_address(
					default_destination(),
					asset,
					amount,
					request_id,
					DEFAULT_STROOP_FEE_PER_OPERATION,
					false,
				)
				.await
				.expect("it should return a success");

			assert!(!response.hash.to_vec().is_empty());
			assert!(response.ledger() > 0);
		});

		let wallet_clone2 = wallet.clone();
		let second_job = tokio::spawn(async move {
			let asset = StellarAsset::native();
			let amount = 50;
			let request_id = [1u8; 32];

			let result = wallet_clone2
				.write()
				.await
				.send_payment_to_address(
					default_destination(),
					asset,
					amount,
					request_id,
					DEFAULT_STROOP_FEE_PER_OPERATION,
					false,
				)
				.await;

			let transaction_response = result.expect("should return a transaction response");
			assert!(!transaction_response.hash.to_vec().is_empty());
			assert!(transaction_response.ledger() > 0);
		});

		let _ = tokio::join!(first_job, second_job);

		wallet.read().await.cache.remove_dir();
	}

	#[tokio::test]
	#[serial]
	async fn sending_payment_using_claimable_balance_works() {
		let wallet =
			wallet_with_storage("resources/sending_payment_using_claimable_balance_works").clone();
		let mut wallet = wallet.write().await;

		// let's cleanup, just to make sure.
		wallet.cache.remove_all_tx_envelopes();

		let amount = 10_000; // in the response, value is 0.0010000.
		let request_id = [1u8; 32];

		let response = wallet
			.send_payment_to_address(
				default_destination(),
				default_usdc_asset(),
				amount,
				request_id,
				DEFAULT_STROOP_FEE_PER_OPERATION,
				true,
			)
			.await
			.expect("payment should work");

		let operation_results = response
			.get_successful_operations_result()
			.expect("should return a vec of size 1");
		// since only 1 operation was performed
		assert_eq!(operation_results.len(), 1);

		match operation_results.first().expect("should return 1") {
			OperationResult::OpInner(OperationResultTr::CreateClaimableBalance(
				CreateClaimableBalanceResult::CreateClaimableBalanceSuccess(id),
			)) => {
				// check existence of claimable balance.
				let HorizonClaimableBalanceResponse { claimable_balance } = wallet
					.client
					.get_claimable_balance(id.clone(), wallet.is_public_network)
					.await
					.expect("should return a response");

				assert_eq!(claimable_balance.sponsor, wallet.get_public_key().to_encoding());

				assert_eq!(&claimable_balance.amount, "0.0010000".as_bytes());

				assert_eq!(claimable_balance.claimants.len(), 1);

				let claimant =
					claimable_balance.claimants.first().expect("should return a claimant");

				assert_eq!(claimant.destination, default_destination().to_encoding());
			},
			other => {
				panic!("wrong operation result: {other:?}");
			},
		}

		wallet.cache.remove_dir();
	}

	#[tokio::test]
	#[serial]
	async fn sending_payment_using_create_account_works() {
		let inactive_secret_key = "SARVWH4LUAR3K5URYJY7DQLXURZUPEBNJYYPMZDRAZWNCQGYIKHPYXC7";
		let destination_secret_key =
			SecretKey::from_encoding(inactive_secret_key).expect("should return a secret key");

		let storage_path = "resources/sending_payment_using_claimable_balance_works";

		let wallet = wallet_with_storage(storage_path);
		let mut wallet = wallet.write().await;

		// let's cleanup, just to make sure.
		wallet.cache.remove_all_tx_envelopes();

		// sending enough amount to be able to perform account merge.
		let amount = 200_000_000;
		let request_id = [1u8; 32];

		let response = wallet
			.send_payment_to_address(
				destination_secret_key.get_public().clone(),
				StellarAsset::AssetTypeNative,
				amount,
				request_id,
				DEFAULT_STROOP_FEE_PER_OPERATION,
				true,
			)
			.await
			.expect("should return a transaction response");

		let operations_results =
			response.get_successful_operations_result().expect("should return a value");
		// since only 1 operation was performed
		assert_eq!(operations_results.len(), 1);

		match operations_results.first().expect("should return 1") {
			OperationResult::OpInner(OperationResultTr::CreateAccount(
				CreateAccountResult::CreateAccountSuccess,
			)) => {
				// since the createaccount operation is a success, make sure to delete the same
				// account to be able to reuse it once this test runs again.
				// DO NOT EDIT THIS PORTION unless necessary.

				// new wallet created, with the previous destination address acting as "SOURCE".
				let temp_wallet =
					wallet_with_secret_key_for_storage(storage_path, inactive_secret_key);
				let mut temp_wallet = temp_wallet.write().await;

				// returning back stellar stroops to `wallet`
				let secret_key = SecretKey::from_encoding(STELLAR_VAULT_SECRET_KEY)
					.expect("should return alright");

				// merging the `temp_wallet` to `wallet`
				let _ = temp_wallet
					.merge_account(secret_key.get_public().clone())
					.await
					.expect("should return a response");

				// the account of temp wallet should not exist anymore, as it merged to
				// `STELLAR_VAULT_SECRET_KEY`.
				assert!(!temp_wallet.is_account_exist().await);

				temp_wallet.cache.remove_dir();
			},
			other => {
				panic!("wrong result: {other:?}");
			},
		}

		wallet.cache.remove_dir();
	}

	#[tokio::test]
	#[serial]
	async fn sending_payment_works() {
		let wallet = wallet_with_storage("resources/sending_payment_works");
		let asset = StellarAsset::native();
		let amount = 100;
		let request_id = [0u8; 32];

		let result = wallet
			.write()
			.await
			.send_payment_to_address(
				default_destination(),
				asset,
				amount,
				request_id,
				DEFAULT_STROOP_FEE_PER_OPERATION,
				false,
			)
			.await;

		assert!(result.is_ok());
		let transaction_response = result.unwrap();
		assert!(!transaction_response.hash.to_vec().is_empty());
		assert!(transaction_response.ledger() > 0);
		wallet.read().await.cache.remove_dir();
	}

	#[cfg(all(test, not(feature = "testing-utils")))]
	#[tokio::test]
	#[serial]
	async fn sending_payment_to_self_not_valid() {
		let wallet = wallet_with_storage("resources/sending_payment_to_self_not_valid").clone();
		let mut wallet = wallet.write().await;

		// let's cleanup, just to make sure.
		wallet.cache.remove_all_tx_envelopes();

		let destination = wallet.secret_key.get_public().clone();

		match wallet
			.send_payment_to_address(
				destination,
				StellarAsset::native(),
				10,
				[0u8; 32],
				DEFAULT_STROOP_FEE_PER_OPERATION,
				false,
			)
			.await
		{
			Err(Error::SelfPaymentError) => {
				assert!(true);
			},
			other => {
				panic!("failed to return SelfPaymentError: {other:?}");
			},
		}
	}

	#[tokio::test]
	#[serial]
	async fn sending_correct_payment_after_incorrect_payment_works() {
		let wallet =
			wallet_with_storage("resources/sending_correct_payment_after_incorrect_payment_works")
				.clone();
		let mut wallet = wallet.write().await;

		// let's cleanup, just to make sure.
		wallet.cache.remove_all_tx_envelopes();

		let asset = StellarAsset::native();
		let amount = 1000;
		let request_id = [0u8; 32];
		let correct_amount_that_should_not_fail = 100;
		let incorrect_amount_that_should_fail = 0;

		let response = wallet
			.send_payment_to_address(
				default_destination(),
				asset.clone(),
				amount,
				request_id,
				correct_amount_that_should_not_fail,
				false,
			)
			.await;

		assert!(response.is_ok());

		let err_insufficient_fee = wallet
			.send_payment_to_address(
				default_destination(),
				asset.clone(),
				amount,
				request_id,
				incorrect_amount_that_should_fail,
				false,
			)
			.await;

		assert!(err_insufficient_fee.is_err());
		match err_insufficient_fee.unwrap_err() {
			Error::HorizonSubmissionError { title: _, status: _, reason, envelope_xdr: _ } => {
				assert_eq!(reason, "tx_insufficient_fee: []");
			},
			_ => assert!(false),
		}

		let tx_response = wallet
			.send_payment_to_address(
				default_destination(),
				asset.clone(),
				amount,
				request_id,
				correct_amount_that_should_not_fail,
				false,
			)
			.await;

		assert!(tx_response.is_ok());

		wallet.cache.remove_dir();
	}

	#[tokio::test]
	#[serial]
	async fn resubmit_transactions_works() {
		let wallet = wallet_with_storage("resources/resubmit_transactions_works").clone();
		let mut wallet = wallet.write().await;

		// let's send a successful transaction first

		let asset = StellarAsset::native();
		let amount = 1001;
		let request_id = [0u8; 32];

		let response = wallet
			.send_payment_to_address(
				default_destination(),
				asset.clone(),
				amount,
				request_id,
				DEFAULT_STROOP_FEE_PER_OPERATION,
				false,
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
			.create_payment_envelope(
				default_destination(),
				asset.clone(),
				amount,
				request_id,
				DEFAULT_STROOP_FEE_PER_OPERATION,
				seq_number,
			)
			.expect("should return an envelope");

		// let's save this in storage
		let _ = wallet.cache.save_tx_envelope(bad_envelope.clone()).expect("should save.");

		// create a successful transaction
		let request_id = [2u8; 32];
		let good_envelope = wallet
			.create_payment_envelope(
				default_destination(),
				asset,
				amount,
				request_id,
				DEFAULT_STROOP_FEE_PER_OPERATION,
				seq_number + 1,
			)
			.expect("should return an envelope");

		// let's save this in storage
		let _ = wallet.cache.save_tx_envelope(good_envelope.clone()).expect("should save");

		// let's resubmit these 2 transactions
		let receivers = wallet.resubmit_transactions_from_cache().await;
		assert_eq!(receivers.len(), 2);

		// a count on how many txs passed, and how many failed.
		let mut passed_count = 0;
		let mut failed_count = 0;

		for receiver in receivers {
			match &receiver.await {
				Ok(Ok(env)) => {
					assert_eq!(env.envelope_xdr, good_envelope.to_base64_xdr());
					passed_count += 1;
				},
				Ok(Err(Error::HorizonSubmissionError {
					title: _,
					status: _,
					reason,
					envelope_xdr: _,
				})) => {
					assert_eq!(reason, "tx_bad_seq: []");
					failed_count += 1;
				},
				other => {
					panic!("other result was received: {other:?}")
				},
			}
		}

		// 1 should pass, and 1 should fail.
		assert_eq!(passed_count, 1);
		assert_eq!(failed_count, 1);

		wallet.cache.remove_dir();
	}
}
