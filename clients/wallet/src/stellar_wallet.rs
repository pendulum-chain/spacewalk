use std::{ops::Deref, sync::Arc, time::Duration};

use async_trait::async_trait;
use substrate_stellar_sdk::{Hash, PublicKey, SecretKey};
use tokio::{
	sync::{Mutex, RwLock},
	time::sleep,
};

use crate::{
	error::Error,
	horizon::{HorizonFetcher, PagingToken},
};

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
	pub fn from_secret_encoded(x: &String) -> Result<Self, Error> {
		let secret_key = SecretKey::from_encoding(x).map_err(|e| Error::InvalidSecretKey)?;

		let wallet = StellarWallet { secret_key, is_public_network: false };
		Ok(wallet)
	}

	pub fn set_is_public_network(&mut self, is_public_network: bool) {
		self.is_public_network = is_public_network;
	}

	pub fn get_public_key_raw(&self) -> StellarPublicKeyRaw {
		self.secret_key.get_public().clone().into_binary()
	}

	pub fn get_public_key(&self) -> PublicKey {
		self.secret_key.get_public().clone()
	}

	pub fn is_public_network(&self) -> bool {
		self.is_public_network
	}
}
