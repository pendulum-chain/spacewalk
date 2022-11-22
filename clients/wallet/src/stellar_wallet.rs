use substrate_stellar_sdk::SecretKey;
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
}

impl StellarWallet {
	pub fn from_secret_encoded(x: &String) -> Result<Self, Error> {
		let secret_key = SecretKey::from_encoding(x).map_err(|e| Error::InvalidSecretKey)?;

		let wallet = StellarWallet { secret_key };
		Ok(wallet)
	}

	pub fn get_public_key_raw(&self) -> StellarPublicKeyRaw {
		self.secret_key.get_public().clone().into_binary()
	}
}
