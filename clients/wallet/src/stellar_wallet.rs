use substrate_stellar_sdk::{Hash, PublicKey, SecretKey};
use thiserror::Error;

pub type StellarPublicKeyRaw = [u8; 32];

#[derive(PartialEq, Eq, Clone, Debug, Error)]
pub enum Error {
	#[error("Server returned rpc error")]
	InvalidSecretKey,
}

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

	pub async fn send_payment_to_address(
		&self,
		address: PublicKey,
		amount: u64,
		memo_hash: Hash,
	) -> Result<(), Error> {
		// todo!()
		Ok(())
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
}
