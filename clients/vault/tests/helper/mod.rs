mod constants;
mod helper;

pub use constants::*;
pub use helper::*;

use async_trait::async_trait;
use lazy_static::lazy_static;
use primitives::CurrencyId;
use runtime::{
	integration::{
		set_exchange_rate_and_wait, setup_provider,
	},
	types::FixedU128,
	SpacewalkParachain, VaultId,
};
use sp_arithmetic::FixedPointNumber;
use sp_keyring::AccountKeyring;
use std::{future::Future, sync::Arc};
use std::process::Child;
use stellar_relay_lib::StellarOverlayConfig;
use tokio::sync::RwLock;
use runtime::integration::{default_root_provider, start_chain};
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
	execute: impl FnOnce(ArcRwLock<StellarWallet>, ArcRwLock<StellarWallet>) -> F,
) -> R
where
	F: Future<Output = R>,
{
	service::init_subscriber();
	// Has to be Bob because he is set as `authorized_oracle` in the genesis config
	let (parachain_rpc, tmp_dir) = default_root_provider(AccountKeyring::Bob).await;

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

	execute(vault_wallet, user_wallet).await
}

pub async fn test_with_vault<F, R>(
	execute: impl FnOnce(
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
	let _parachain_runner: Child = start_chain().await.unwrap();
	service::init_subscriber();
	let (parachain_rpc, tmp_dir) = default_root_provider(AccountKeyring::Bob).await;

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

	let vault_provider = setup_provider(AccountKeyring::Charlie).await;
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

	let oracle_agent = start_oracle_agent(CFG.clone(), &SECRET_KEY)
		.await
		.expect("failed to start agent");
	let oracle_agent = Arc::new(oracle_agent);

	execute(vault_wallet, user_wallet, oracle_agent, vault_id, vault_provider).await
}
