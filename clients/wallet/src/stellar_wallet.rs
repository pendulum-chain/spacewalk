use reqwest::Client;
use std::{fmt::Formatter, sync::Arc};

use primitives::stellar::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::SequenceNumber,
	Asset as StellarAsset, Operation, PublicKey, SecretKey, StellarTypeToString, Transaction,
	TransactionEnvelope,
};
use tokio::sync::Mutex;

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
use primitives::{StellarPublicKeyRaw, StellarStroops, TransactionEnvelopeExt};


#[derive(Clone)]
pub struct StellarWallet {
	secret_key: SecretKey,
	is_public_network: bool,
	/// Used to make sure that only one transaction is submitted at a time,
	/// so that the transaction is not rejected due to an outdated sequence number.
	/// Releasing the lock ensures the sequence number of the account
	/// has been increased on the network.
	pub(crate) transaction_submission_lock: Arc<Mutex<()>>,
	/// Used for caching Stellar transactions before they get submitted.
	/// Also used for caching the latest cursor to page through Stellar transactions in horizon
	cache: WalletStateStorage,

	/// maximum retry attempts for submitting a transaction before switching to a fallback url
	max_retry_attempts_before_fallback: u8,

	/// the waiting time (in seconds) for retrying.
	max_backoff_delay: u16,

	/// a client to connect to Horizon
	pub(crate) client: Client,
}

impl StellarWallet {
	/// if the user doesn't define the maximum number of retry attempts for 500 internal server
	/// error, this will be the default.
	pub(crate) const DEFAULT_MAX_RETRY_ATTEMPTS_BEFORE_FALLBACK: u8 = 3;

	pub(crate) const DEFAULT_MAX_BACKOFF_DELAY_IN_SECS: u16 = 600;

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
}

// getters and other derivations
impl StellarWallet {
	pub fn max_backoff_delay(&self) -> u16 {
		self.max_backoff_delay
	}

	pub fn max_retry_attempts_before_fallback(&self) -> u8 {
		self.max_retry_attempts_before_fallback
	}

	pub fn public_key_raw(&self) -> StellarPublicKeyRaw {
		self.secret_key.get_public().clone().into_binary()
	}

	pub fn public_key(&self) -> PublicKey {
		self.secret_key.get_public().clone()
	}

	pub fn secret_key(&self) -> SecretKey {
		self.secret_key.clone()
	}

	pub fn is_public_network(&self) -> bool {
		self.is_public_network
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
				self.public_key(),
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
			self.client.get_account(self.public_key(), self.is_public_network).await?;
		Ok(account.balances)
	}

	pub async fn get_sequence(&self) -> Result<SequenceNumber, Error> {
		let account =
			self.client.get_account(self.public_key(), self.is_public_network).await?;
		Ok(account.sequence)
	}

}

// cache operations
impl StellarWallet {

	pub fn last_cursor(&self) -> PagingToken {
		self.cache.get_last_cursor()
	}

	pub fn save_cursor(&self, paging_token: PagingToken) -> Result<(), Error> {
		self.cache.save_cursor(paging_token)
	}

	#[doc(hidden)]
	#[cfg(any(test, feature = "testing-utils"))]
	pub fn remove_cache_dir(&self) {
		self.cache.remove_dir()
	}

	#[doc(hidden)]
	#[cfg(any(test, feature = "testing-utils"))]
	pub fn remove_tx_envelopes_from_cache(&self) {
		self.cache.remove_all_tx_envelopes()
	}

	pub fn get_tx_envelopes_from_cache(&self) -> Result<(Vec<TransactionEnvelope>, Vec<Error>), Vec<Error>> {
		self.cache.get_tx_envelopes()
	}

	pub fn remove_tx_envelope_from_cache(&self, sequence:SequenceNumber) -> Result<(), Error> {
		self.cache.remove_tx_envelope(sequence)
	}

	pub fn save_tx_envelope_to_cache(&self, tx_envelope: TransactionEnvelope) -> Result<(), Error> {
		self.cache.save_tx_envelope(tx_envelope)
	}
}


// send/submit functions of StellarWallet
impl StellarWallet {

	/// Returns a TransactionResponse after submitting transaction envelope to Stellar,
	/// Else an Error.
	pub async fn submit_transaction(
		&self,
		envelope: TransactionEnvelope,
	) -> Result<TransactionResponse, Error> {
		let sequence = &envelope.sequence_number().ok_or(Error::cache_error_with_env(
			CacheErrorKind::UnknownSequenceNumber,
			envelope.clone(),
		))?;

		let _ = self.save_tx_envelope_to_cache(envelope.clone());

		let submission_result = self
			.client
			.submit_transaction(
				envelope.clone(),
				self.is_public_network(),
				self.max_retry_attempts_before_fallback(),
				self.max_backoff_delay(),
			)
			.await;

		let _ = self.remove_tx_envelope_from_cache(*sequence);

		submission_result
	}

	pub(crate) fn sign_and_create_envelope(&self, tx:Transaction) -> Result<TransactionEnvelope, Error> {
		// convert to envelope
		let mut envelope = tx.into_transaction_envelope();
		self.sign_envelope(&mut envelope)?;

		Ok(envelope)
	}

	pub(crate) fn sign_envelope(&self, envelope:&mut TransactionEnvelope) -> Result<(),Error> {
		let network: &Network =
			if self.is_public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

		envelope
			.sign(network, vec![&self.secret_key()])
			.map_err(|_e| Error::SignEnvelopeError)?;

		Ok(())
	}

	pub(crate) fn create_envelope(
		&self,
		request_id: [u8; 32],
		stroop_fee_per_operation: u32,
		next_sequence_number: SequenceNumber,
		operations: Vec<Operation>,
	) -> Result<TransactionEnvelope, Error> {
		let public_key = self.public_key();

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
		self.sign_and_create_envelope(transaction)
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
					self.public_key(),
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
				self.public_key(),
			)?
		};

		self.send_to_address(request_id, stroop_fee_per_operation, vec![payment_op])
			.await
	}

	pub(crate) async fn send_to_address(
		&mut self,
		request_id: [u8; 32],
		stroop_fee_per_operation: u32,
		operations: Vec<Operation>,
	) -> Result<TransactionResponse, Error> {
		let _ = self.transaction_submission_lock.lock().await;

		let account =
			self.client.get_account(self.public_key(), self.is_public_network).await?;
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
	use primitives::stellar::Asset as StellarAsset;
	use primitives::stellar::types::{CreateAccountResult, CreateClaimableBalanceResult, OperationResult, OperationResultTr};
	use serial_test::serial;
	use crate::error::Error;
	use crate::horizon::HorizonClient;
	use crate::horizon::responses::HorizonClaimableBalanceResponse;
	use crate::mock::*;
	use crate::StellarWallet;

	#[test]
	fn test_add_backoff_delay() {
		let wallet = StellarWallet::from_secret_encoded_with_cache(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			IS_PUBLIC_NETWORK,
			"resources/test_add_backoff_delay".to_owned(),
		)
			.expect("should return a wallet");

		assert_eq!(wallet.max_backoff_delay(), StellarWallet::DEFAULT_MAX_BACKOFF_DELAY_IN_SECS);

		// too big backoff delay
		let expected_max_backoff_delay = 800;
		let new_wallet = wallet.with_max_backoff_delay(expected_max_backoff_delay);
		assert_ne!(new_wallet.max_backoff_delay(), expected_max_backoff_delay);

		let expected_max_backoff_delay = 300;
		let new_wallet = new_wallet.with_max_backoff_delay(expected_max_backoff_delay);
		assert_eq!(new_wallet.max_backoff_delay(), expected_max_backoff_delay);

		new_wallet.remove_cache_dir();
	}

	#[test]
	fn test_add_retry_attempt() {
		let wallet = StellarWallet::from_secret_encoded_with_cache(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			IS_PUBLIC_NETWORK,
			"resources/test_add_retry_attempt".to_owned(),
		)
			.expect("should return an arc rwlock wallet");

		assert_eq!(
			wallet.max_retry_attempts_before_fallback(),
			StellarWallet::DEFAULT_MAX_RETRY_ATTEMPTS_BEFORE_FALLBACK
		);

		let expected_max_retries = 5;
		let new_wallet = wallet.with_max_retry_attempts_before_fallback(expected_max_retries);
		assert_eq!(new_wallet.max_retry_attempts_before_fallback(), expected_max_retries);

		new_wallet.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn test_locking_submission() {
		let wallet = wallet_with_storage("resources/test_locking_submission")
			.expect("should return an arc rwlock wallet")
			.clone();
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

		wallet.read().await.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn sending_payment_using_claimable_balance_works() {
		let wallet = wallet_with_storage("resources/sending_payment_using_claimable_balance_works")
			.expect("should return an arc rwlock wallet")
			.clone();
		let mut wallet = wallet.write().await;

		// let's cleanup, just to make sure.
		wallet.remove_tx_envelopes_from_cache();

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
					.get_claimable_balance(id.clone(), wallet.is_public_network())
					.await
					.expect("should return a response");

				assert_eq!(claimable_balance.sponsor, wallet.public_key().to_encoding());

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

		wallet.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn sending_payment_using_create_account_works() {
		let inactive_secret_key = "SARVWH4LUAR3K5URYJY7DQLXURZUPEBNJYYPMZDRAZWNCQGYIKHPYXC7";
		let destination_secret_key = secret_key_from_encoding(inactive_secret_key);
		let storage_path = "resources/sending_payment_using_claimable_balance_works";

		let wallet = wallet_with_storage(storage_path).expect("should return an arc rwlock wallet");
		let mut wallet = wallet.write().await;

		// let's cleanup, just to make sure.
		wallet.remove_tx_envelopes_from_cache();

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
					wallet_with_secret_key_for_storage(storage_path, inactive_secret_key)
						.expect("should return a wallet instance");
				let mut temp_wallet = temp_wallet.write().await;

				// returning back stellar stroops to `wallet`
				let secret_key = secret_key_from_encoding(STELLAR_VAULT_SECRET_KEY);

				// merging the `temp_wallet` to `wallet`
				let _ = temp_wallet
					.merge_account(secret_key.get_public().clone())
					.await
					.expect("should return a response");

				// the account of temp wallet should not exist anymore, as it merged to
				// `STELLAR_VAULT_SECRET_KEY`.
				assert!(!temp_wallet.is_account_exist().await);

				temp_wallet.remove_cache_dir();
			},
			other => {
				panic!("wrong result: {other:?}");
			},
		}

		wallet.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn sending_payment_works() {
		let wallet = wallet_with_storage("resources/sending_payment_works")
			.expect("should return an arc rwlock wallet");
		let asset = StellarAsset::native();
		let amount = 100;
		let request_id = [0u8; 32];

		let transaction_response = wallet
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
			.expect("should return ok");

		assert!(!transaction_response.hash.to_vec().is_empty());
		assert!(transaction_response.ledger() > 0);
		wallet.read().await.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn sending_payment_to_self_not_valid() {
		let wallet = wallet_with_storage("resources/sending_payment_to_self_not_valid")
			.expect("should return an arc rwlock wallet")
			.clone();
		let mut wallet = wallet.write().await;

		// let's cleanup, just to make sure.
		wallet.remove_tx_envelopes_from_cache();

		let destination = wallet.public_key().clone();

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

		wallet.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn sending_correct_payment_after_incorrect_payment_works() {
		let wallet =
			wallet_with_storage("resources/sending_correct_payment_after_incorrect_payment_works")
				.expect("should return an arc rwlock wallet")
				.clone();
		let mut wallet = wallet.write().await;

		// let's cleanup, just to make sure.
		wallet.remove_tx_envelopes_from_cache();

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
			Error::HorizonSubmissionError {  reason, .. } => {
				assert_eq!(reason, "tx_insufficient_fee");
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

		wallet.remove_tx_envelopes_from_cache();
	}


}