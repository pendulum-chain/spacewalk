mod constants;
mod helper;

pub use constants::*;
pub use helper::*;

use async_trait::async_trait;
use lazy_static::lazy_static;
use primitives::CurrencyId;
use runtime::{integration::{
	default_provider_client, set_exchange_rate_and_wait, setup_provider, SubxtClient,
}, types::FixedU128, SpacewalkParachain, VaultId, ShutdownSender};
use sp_arithmetic::FixedPointNumber;
use sp_keyring::AccountKeyring;
use std::{future::Future, sync::Arc};
use stellar_relay_lib::StellarOverlayConfig;
use tokio::sync::RwLock;
use vault::{
	oracle::{get_test_secret_key, get_test_stellar_relay_config, start_oracle_agent, OracleAgent},
	ArcRwLock,
};
use wallet::StellarWallet;

pub type StellarPublicKey = [u8; 32];

lazy_static! {
	pub static ref CFG: StellarOverlayConfig = get_test_stellar_relay_config(false);
	pub static ref SECRET_KEY: String = get_test_secret_key(false);
	// TODO clean this up by extending the `get_test_secret_key()` function
	pub static ref DESTINATION_SECRET_KEY: String = "SDNQJEIRSA6YF5JNS6LQLCBF2XVWZ2NJV3YLC322RGIBJIJRIRGWKLEF".to_string();
}

#[async_trait]
impl SpacewalkParachainExt for SpacewalkParachain {}

pub async fn test_with<F, R>(
	execute: impl FnOnce(SubxtClient, ArcRwLock<StellarWallet>, ArcRwLock<StellarWallet>) -> F,
) -> R
where
	F: Future<Output = R>,
{
	service::init_subscriber();
	let (client, tmp_dir) = default_provider_client(AccountKeyring::Alice).await;

	// Has to be Bob because he is set as `authorized_oracle` in the genesis config
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;

	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_TESTING_CURRENCY,
		// Set exchange rate to 1:1 with USD
		FixedU128::saturating_from_rational(1u128, 1u128),
	)
	.await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_WRAPPED_CURRENCY,
		// Set exchange rate to 10:1 with USD
		FixedU128::saturating_from_rational(1u128, 10u128),
	)
	.await;

	set_exchange_rate_and_wait(
		&parachain_rpc,
		LESS_THAN_4_CURRENCY_CODE,
		// Set exchange rate to 10:1 with USD
		FixedU128::saturating_from_rational(1u128, 10u128),
	)
	.await;

	set_exchange_rate_and_wait(
		&parachain_rpc,
		CurrencyId::StellarNative,
		// Set exchange rate to 10:1 with USD
		FixedU128::saturating_from_rational(1u128, 10u128),
	)
	.await;

	let path = tmp_dir.path().to_str().expect("should return a string").to_string();
	let vault_wallet = Arc::new(RwLock::new(
		StellarWallet::from_secret_encoded_with_cache(
			&SECRET_KEY,
			CFG.is_public_network(),
			path.clone(),
		)
		.unwrap(),
	));

	let user_wallet = Arc::new(RwLock::new(
		StellarWallet::from_secret_encoded_with_cache(
			&DESTINATION_SECRET_KEY,
			CFG.is_public_network(),
			path,
		)
		.unwrap(),
	));

	execute(client, vault_wallet, user_wallet).await
}

pub async fn test_with_vault<F, R>(
	execute: impl FnOnce(
		SubxtClient,
		ArcRwLock<StellarWallet>,
		ArcRwLock<StellarWallet>,
		Arc<OracleAgent>,
		VaultId,
		SpacewalkParachain,
	) -> F,
) -> R
where
	F: Future<Output = R>,
{
	service::init_subscriber();
	let (client, tmp_dir) = default_provider_client(AccountKeyring::Alice).await;

	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_TESTING_CURRENCY,
		FixedU128::saturating_from_rational(1u128, 1u128),
	)
	.await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_WRAPPED_CURRENCY,
		// Set exchange rate to 10:1 with USD
		FixedU128::saturating_from_rational(1u128, 10u128),
	)
	.await;

	set_exchange_rate_and_wait(
		&parachain_rpc,
		LESS_THAN_4_CURRENCY_CODE,
		// Set exchange rate to 100:1 with USD
		FixedU128::saturating_from_rational(1u128, 10u128),
	)
	.await;

	set_exchange_rate_and_wait(
		&parachain_rpc,
		CurrencyId::StellarNative,
		// Set exchange rate to 10:1 with USD
		FixedU128::saturating_from_rational(1u128, 10u128),
	)
	.await;

	let vault_provider = setup_provider(client.clone(), AccountKeyring::Charlie).await;
	let vault_id = VaultId::new(
		AccountKeyring::Charlie.into(),
		DEFAULT_TESTING_CURRENCY,
		DEFAULT_WRAPPED_CURRENCY,
	);

	let path = tmp_dir.path().to_str().expect("should return a string").to_string();
	let vault_wallet = Arc::new(RwLock::new(
		StellarWallet::from_secret_encoded_with_cache(
			&SECRET_KEY,
			CFG.is_public_network(),
			path.clone(),
		)
		.unwrap(),
	));

	let user_wallet = Arc::new(RwLock::new(
		StellarWallet::from_secret_encoded_with_cache(
			&DESTINATION_SECRET_KEY,
			CFG.is_public_network(),
			path,
		)
		.unwrap(),
	));

	let shutdown_tx = ShutdownSender::new();
	let oracle_agent = start_oracle_agent(CFG.clone(), &SECRET_KEY, shutdown_tx)
		.await
		.expect("failed to start agent");
	let oracle_agent = Arc::new(oracle_agent);

	execute(client, vault_wallet, user_wallet, oracle_agent, vault_id, vault_provider).await
}
