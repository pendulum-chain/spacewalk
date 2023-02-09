use std::{fmt::Formatter, sync::Arc};

use substrate_stellar_sdk::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::Preconditions,
	Asset, Hash, Memo, Operation, PublicKey, SecretKey, StroopAmount, Transaction,
	TransactionEnvelope,
};
use tokio::sync::Mutex;

use crate::{
	error::Error,
	horizon::{HorizonClient, PagingToken, TransactionResponse},
	types::StellarPublicKeyRaw,
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
}

impl StellarWallet {
	pub fn from_secret_encoded(
		secret_key: &String,
		is_public_network: bool,
	) -> Result<Self, Error> {
		let secret_key =
			SecretKey::from_encoding(secret_key).map_err(|_| Error::InvalidSecretKey)?;

		let wallet = StellarWallet {
			secret_key,
			is_public_network,
			transaction_submission_lock: Arc::new(Mutex::new(())),
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

	pub async fn send_payment_to_address(
		&mut self,
		destination_address: PublicKey,
		asset: Asset,
		stroop_amount: i64,
		memo_hash: Hash,
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
		let mut transaction = Transaction::new(
			self.get_public_key(),
			next_sequence_number,
			Some(stroop_fee_per_operation),
			Preconditions::PrecondNone,
			Some(Memo::MemoHash(memo_hash)),
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

		let transaction_response = horizon_client
			.submit_transaction(envelope.clone(), self.is_public_network)
			.await?;

		Ok((transaction_response, envelope))
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

#[cfg(test)]
mod test {
	use mockall::lazy_static;
	use serial_test::serial;
	use std::sync::Arc;
	use substrate_stellar_sdk::PublicKey;
	use tokio::sync::RwLock;

	use crate::StellarWallet;

	const STELLAR_VAULT_SECRET_KEY: &str =
		"SCV7RZN5XYYMMVSWYCR4XUMB76FFMKKKNHP63UTZQKVM4STWSCIRLWFJ";
	const IS_PUBLIC_NETWORK: bool = false;

	lazy_static! {
		static ref WALLET: Arc<RwLock<StellarWallet>> = Arc::new(RwLock::new(
			StellarWallet::from_secret_encoded(
				&STELLAR_VAULT_SECRET_KEY.to_string(),
				IS_PUBLIC_NETWORK,
			)
			.unwrap()
		));
	}

	#[tokio::test]
	async fn test_locking_submission() {
		let wallet_clone = WALLET.clone();
		let first_job = tokio::spawn(async move {
			let destination = PublicKey::from_encoding(
				"GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA",
			)
			.unwrap();
			let asset = substrate_stellar_sdk::Asset::native();
			let amount = 100;
			let memo_hash = [0u8; 32];

			let result = wallet_clone
				.write()
				.await
				.send_payment_to_address(destination, asset, amount, memo_hash, 100)
				.await;

			assert!(result.is_ok());
			let (transaction_response, _) = result.unwrap();
			assert!(!transaction_response.hash.to_vec().is_empty());
			assert!(transaction_response.ledger() > 0);
		});

		let wallet_clone2 = WALLET.clone();
		let second_job = tokio::spawn(async move {
			let destination = PublicKey::from_encoding(
				"GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA",
			)
			.unwrap();
			let asset = substrate_stellar_sdk::Asset::native();
			let amount = 50;
			let memo_hash = [1u8; 32];

			let result = wallet_clone2
				.write()
				.await
				.send_payment_to_address(destination, asset, amount, memo_hash, 100)
				.await;

			assert!(result.is_ok());
			let (transaction_response, _) = result.unwrap();
			assert!(!transaction_response.hash.to_vec().is_empty());
			assert!(transaction_response.ledger() > 0);
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
		let memo_hash = [0u8; 32];

		let result = WALLET
			.write()
			.await
			.send_payment_to_address(destination, asset, amount, memo_hash, 100)
			.await;

		assert!(result.is_ok());
		let (transaction_response, _) = result.unwrap();
		assert!(!transaction_response.hash.to_vec().is_empty());
		assert!(transaction_response.ledger() > 0);
	}

	#[tokio::test]
	#[serial]
	async fn sending_correct_payment_after_incorrect_payment_works() {
		let wallet = WALLET.clone();
		let mut wallet = wallet.write().await;

		let destination =
			PublicKey::from_encoding("GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA")
				.unwrap();
		let asset = substrate_stellar_sdk::Asset::native();
		let amount = 1000;
		let memo_hash = [0u8; 32];
		let correct_amount_that_should_not_fail = 100;
		let incorrect_amount_that_should_fail = 0;

		let ok_transaction_sent = wallet
			.send_payment_to_address(
				destination.clone(),
				asset.clone(),
				amount,
				memo_hash,
				correct_amount_that_should_not_fail,
			)
			.await;

		assert!(ok_transaction_sent.is_ok());

		let err_insufficient_fee = wallet
			.send_payment_to_address(
				destination.clone(),
				asset.clone(),
				amount,
				memo_hash,
				incorrect_amount_that_should_fail,
			)
			.await;

		assert!(!err_insufficient_fee.is_ok());

		let ok_sequence_number = wallet
			.send_payment_to_address(
				destination.clone(),
				asset.clone(),
				amount,
				memo_hash,
				correct_amount_that_should_not_fail,
			)
			.await;

		assert!(ok_sequence_number.is_ok());
	}
}
