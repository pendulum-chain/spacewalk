use std::fmt::Formatter;

use substrate_stellar_sdk::{
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::Preconditions,
	Asset, Hash, Memo, Operation, PublicKey, SecretKey, StroopAmount, Transaction,
	TransactionEnvelope,
};

use crate::{
	error::Error,
	horizon::{HorizonClient, PagingToken, TransactionResponse},
	types::StellarPublicKeyRaw,
};

#[derive(Clone, PartialEq, Eq)]
pub struct StellarWallet {
	secret_key: SecretKey,
	is_public_network: bool,
	/// The last used account sequence number. We need this when sending multiple transactions in a
	/// batch because otherwise the transactions would be rejected since the sequence number is not
	/// increased on the network.
	last_account_sequence: i64,
}

impl StellarWallet {
	pub fn from_secret_encoded(
		secret_key: &String,
		is_public_network: bool,
	) -> Result<Self, Error> {
		let secret_key =
			SecretKey::from_encoding(secret_key).map_err(|_| Error::InvalidSecretKey)?;

		let wallet = StellarWallet { secret_key, is_public_network, last_account_sequence: 0 };
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
		let account_id =
			std::str::from_utf8(&public_key_encoded).map_err(Error::Utf8Error)?;

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
		let horizon_client = reqwest::Client::new();

		let public_key_encoded = self.get_public_key().to_encoding();
		let account_id_string =
			std::str::from_utf8(&public_key_encoded).map_err(Error::Utf8Error)?;
		let account = horizon_client.get_account(account_id_string, self.is_public_network).await?;
		// Either use the local one or the one from the network depending on which one is higher.
		let next_sequence_number = if self.last_account_sequence > account.sequence {
			self.last_account_sequence + 1
		} else {
			account.sequence + 1
		};

		self.last_account_sequence = next_sequence_number;

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
	use substrate_stellar_sdk::PublicKey;

	use crate::StellarWallet;

	const STELLAR_SECRET_ENCODED: &str = "SCV7RZN5XYYMMVSWYCR4XUMB76FFMKKKNHP63UTZQKVM4STWSCIRLWFJ";

	#[tokio::test]
	async fn sending_payment_works() {
		let mut wallet =
			StellarWallet::from_secret_encoded(&STELLAR_SECRET_ENCODED.to_string(), false).unwrap();

		let destination =
			PublicKey::from_encoding("GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA")
				.unwrap();
		let asset = substrate_stellar_sdk::Asset::native();
		let amount = 100;
		let memo_hash = [0u8; 32];

		let result = wallet.send_payment_to_address(destination, asset, amount, memo_hash, 1).await;

		assert!(result.is_ok());
		let (transaction_response, _) = result.unwrap();
		assert!(!transaction_response.hash.to_vec().is_empty());
		assert!(transaction_response.ledger > 0);
	}
}
