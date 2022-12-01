use async_trait::async_trait;
use substrate_stellar_sdk::{PublicKey, SecretKey};

use crate::error::Error;

pub type StellarPublicKeyRaw = [u8; 32];

#[async_trait]
pub trait Watcher: Send + Sync {
	async fn watch_slot(&self, slot: u128) -> Result<(), Error>;
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

	pub fn get_public_key(&self) -> PublicKey {
		self.secret_key.get_public().clone()
	}
}
