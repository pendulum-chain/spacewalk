mod constants;
mod helper;

pub use constants::*;
pub use helper::*;

use async_trait::async_trait;
use lazy_static::lazy_static;
use primitives::CurrencyId;
use runtime::{
	integration::{
		default_provider_client, set_exchange_rate_and_wait, setup_provider, SubxtClient,
	},
	types::FixedU128,
	ShutdownSender, SpacewalkParachain, VaultId,
};
use sp_arithmetic::FixedPointNumber;
use sp_keyring::AccountKeyring;
use std::{future::Future, sync::Arc};
use stellar_relay_lib::StellarOverlayConfig;
use tokio::sync::RwLock;
use vault::{
	oracle::{get_testnet_secret_key, random_stellar_relay_config, start_oracle_agent, OracleAgent},
	ArcRwLock,
};
use wallet::StellarWallet;

pub type StellarPublicKey = [u8; 32];

#[async_trait]
impl SpacewalkParachainExt for SpacewalkParachain {}

lazy_static! {
	pub static ref CFG: StellarOverlayConfig = random_stellar_relay_config(false);
	pub static ref SECRET_KEY: String = get_testnet_secret_key(true);
	pub static ref DESTINATION_SECRET_KEY: String = get_testnet_secret_key(false);
	pub static ref ONE_TO_ONE_RATIO: FixedU128 = FixedU128::saturating_from_rational(1u128, 1u128);
	pub static ref TEN_TO_ONE_RATIO: FixedU128 = FixedU128::saturating_from_rational(1u128, 10u128);
}

async fn initialize_wallets(
	vault_stellar_secret: &String,
	user_stellar_secret: &String,
	path: String,
	config: StellarOverlayConfig,
) -> (ArcRwLock<StellarWallet>, ArcRwLock<StellarWallet>) {
	let vault_wallet = Arc::new(RwLock::new(
		StellarWallet::from_secret_encoded_with_cache(
			vault_stellar_secret.as_str(),
			config.is_public_network(),
			path.clone(),
		)
		.unwrap(),
	));
	let user_wallet = Arc::new(RwLock::new(
		StellarWallet::from_secret_encoded_with_cache(
			user_stellar_secret.as_str(),
			config.is_public_network(),
			path,
		)
		.unwrap(),
	));
	(vault_wallet, user_wallet)
}

async fn setup_chain_providers(
	is_public_network: bool,
) -> (SubxtClient, ArcRwLock<StellarWallet>, ArcRwLock<StellarWallet>) {
	let (client, tmp_dir) = default_provider_client(AccountKeyring::Alice, is_public_network).await;

	// Has to be Bob because he is set as `authorized_oracle` in the genesis config
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;

	let default_wrapped_currency = if is_public_network {
		DEFAULT_WRAPPED_CURRENCY_STELLAR_MAINNET
	} else {
		DEFAULT_WRAPPED_CURRENCY_STELLAR_TESTNET
	};

	set_exchange_rate_and_wait(&parachain_rpc, DEFAULT_TESTING_CURRENCY, *ONE_TO_ONE_RATIO).await;
	set_exchange_rate_and_wait(&parachain_rpc, default_wrapped_currency, *TEN_TO_ONE_RATIO).await;
	set_exchange_rate_and_wait(&parachain_rpc, LESS_THAN_4_CURRENCY_CODE, *TEN_TO_ONE_RATIO).await;
	set_exchange_rate_and_wait(&parachain_rpc, CurrencyId::StellarNative, *TEN_TO_ONE_RATIO).await;

	let path = tmp_dir.path().to_str().expect("should return a string").to_string();

	let stellar_config = random_stellar_relay_config(is_public_network);
	let vault_stellar_secret = get_test_secret_key(is_public_network);
	// TODO set destination secret key in a better way
	let user_stellar_secret = &DESTINATION_SECRET_KEY;

	let (vault_wallet, user_wallet) =
		initialize_wallets(&vault_stellar_secret, &user_stellar_secret, path, stellar_config).await;

	return (client, vault_wallet, user_wallet)
}

pub async fn test_with<F, R>(
	is_public_network: bool,
	execute: impl FnOnce(SubxtClient, ArcRwLock<StellarWallet>, ArcRwLock<StellarWallet>) -> F,
) -> R
where
	F: Future<Output = R>,
{
	service::init_subscriber();
	let (client, vault_wallet, user_wallet) = setup_chain_providers(is_public_network).await;

	execute(client, vault_wallet, user_wallet).await
}

pub async fn test_with_vault<F, R>(
	is_public_network: bool,
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

	let (client, vault_wallet, user_wallet) = setup_chain_providers(is_public_network).await;

	let vault_provider = setup_provider(client.clone(), AccountKeyring::Charlie).await;
	let default_wrapped_currency = if is_public_network {
		DEFAULT_WRAPPED_CURRENCY_STELLAR_MAINNET
	} else {
		DEFAULT_WRAPPED_CURRENCY_STELLAR_TESTNET
	};

	let vault_id = VaultId::new(
		AccountKeyring::Charlie.into(),
		DEFAULT_TESTING_CURRENCY,
		default_wrapped_currency,
	);

	let stellar_config = random_stellar_relay_config(is_public_network);
	let vault_stellar_secret = get_test_secret_key(is_public_network);

	let shutdown_tx = ShutdownSender::new();
	let oracle_agent =
		start_oracle_agent(stellar_config.clone(), &vault_stellar_secret, shutdown_tx)
			.await
			.expect("failed to start agent");
	let oracle_agent = Arc::new(oracle_agent);

	execute(client, vault_wallet, user_wallet, oracle_agent, vault_id, vault_provider).await
}
