use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use substrate_stellar_sdk::{Hash, SecretKey};
use tokio::{
	sync::{Mutex, RwLock},
	time::sleep,
};

use crate::{
	error::Error,
	horizon::{HorizonFetcher, PagingToken},
};

pub type StellarPublicKeyRaw = [u8; 32];
const POLL_INTERVAL: u64 = 5000;

#[async_trait]
pub trait Watcher {
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

	pub async fn listen_for_new_transactions(
		&self,
		targets: Arc<RwLock<Vec<Hash>>>,
		watcher: Arc<RwLock<dyn Watcher>>,
	) -> Result<(), Error> {
		let horizon_client = reqwest::Client::new();
		let mut fetcher = HorizonFetcher::new(
			horizon_client,
			self.secret_key.get_public().clone(),
			self.is_public_network,
		);

		let mut latest_paging_token = 0;
		// Start polling horizon every 5 seconds
		loop {
			if let Ok(new_paging_token) = fetcher
				.fetch_horizon_and_process_new_transactions(
					targets.clone(),
					watcher.clone(),
					latest_paging_token,
				)
				.await
			{
				latest_paging_token = new_paging_token;
			}
			sleep(Duration::from_millis(POLL_INTERVAL)).await;
		}
	}
}
