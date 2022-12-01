
use thiserror::Error;
use substrate_stellar_sdk::{
	horizon::Horizon,
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::Preconditions,
	Asset, Hash, Memo, Operation, PublicKey, SecretKey, StroopAmount, Transaction, XdrCodec,
};

use crate::{error::Error, horizon::HorizonClient, types::StellarPublicKeyRaw};

#[derive(Clone, PartialEq, Debug, Eq)]
pub struct StellarWallet {
	secret_key: SecretKey,
	is_public_network: bool,
}

impl StellarWallet {
	pub fn from_secret_encoded(
		secret_key: &String,
		is_public_network: bool,
	) -> Result<Self, Error> {
		let secret_key =
			SecretKey::from_encoding(secret_key).map_err(|_| Error::InvalidSecretKey)?;

		let wallet = StellarWallet { secret_key, is_public_network };
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

	pub async fn send_payment_to_address(
		&self,
		destination_address: PublicKey,
		asset: Asset,
		stroop_amount: i64,
		memo_hash: Hash,
	) -> Result<(), Error> {
		// todo!()

		let horizon_url = if self.is_public_network {
			"https://horizon.stellar.org"
		} else {
			"https://horizon-testnet.stellar.org"
		};

		// TODO properly fetch the sequence number
		let next_sequence_number = 0;

		let fee_per_operation = 100;

		let mut transaction = Transaction::new(
			self.get_public_key(),
			next_sequence_number,
			Some(fee_per_operation),
			Preconditions::PrecondNone,
			Some(Memo::MemoHash(memo_hash)),
		)
		.map_err(|e| Error::BuildTransactionError)?;

		let amount = StroopAmount(stroop_amount);
		transaction
			.append_operation(
				Operation::new_payment(destination_address, asset, amount)
					.map_err(|e| Error::BuildTransactionError)?
					.set_source_account(self.get_public_key())
					.map_err(|e| Error::BuildTransactionError)?,
			)
			.map_err(|e| Error::BuildTransactionError)?;

		let mut envelope = transaction.into_transaction_envelope();
		let network: &Network =
			if self.is_public_network { &PUBLIC_NETWORK } else { &TEST_NETWORK };

		envelope.sign(network, vec![&self.get_secret_key()]);

		let env_xdr = envelope.to_base64_xdr();
		let xdr_string = std::str::from_utf8(&env_xdr).map_err(|e| Error::BuildTransactionError)?;

		let horizon_client = reqwest::Client::new();
		let submission_response = horizon_client
			.submit_transaction(horizon_url, xdr_string)
			.await
			.map_err(|e| Error::HorizonSubmissionError)?;

		tracing::info!("Response: {:?}", submission_response);

		Ok(())
	}
}

#[cfg(test)]
mod test {
	use substrate_stellar_sdk::PublicKey;

	use crate::StellarWallet;

	const STELLAR_SECRET_ENCODED: &str = "SCV7RZN5XYYMMVSWYCR4XUMB76FFMKKKNHP63UTZQKVM4STWSCIRLWFJ";

	#[tokio::test]
	async fn sending_payment_works() {
		let wallet =
			StellarWallet::from_secret_encoded(&STELLAR_SECRET_ENCODED.to_string(), false).unwrap();

		let destination =
			PublicKey::from_encoding("GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA")
				.unwrap();
		let asset = substrate_stellar_sdk::Asset::native();
		let amount = 100;
		let memo_hash = [0u8; 32];

		let result = wallet.send_payment_to_address(destination, asset, amount, memo_hash).await;

		assert!(result.is_ok());
	}
}
