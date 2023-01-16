#![cfg(test)]

use std::time::Duration;

use sp_keyring::AccountKeyring;

use primitives::{ForeignCurrencyId, StellarPublicKeyRaw};

use crate::{integration::*, FeedValuesEvent, OracleKey, VaultId};

use super::{
	CollateralBalancesPallet, CurrencyId, FixedPointNumber, FixedU128, OraclePallet,
	SecurityPallet, StatusCode, VaultRegistryPallet,
};

const DEFAULT_TESTING_CURRENCY: CurrencyId = CurrencyId::XCM(ForeignCurrencyId::KSM);
const DEFAULT_WRAPPED_CURRENCY: CurrencyId = CurrencyId::AlphaNum4 {
	code: *b"USDC",
	issuer: [
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
};

fn dummy_public_key() -> StellarPublicKeyRaw {
	[0u8; 32]
}

async fn set_exchange_rate(client: SubxtClient) {
	let oracle_provider = setup_provider(client, AccountKeyring::Bob).await;
	let key = OracleKey::ExchangeRate(DEFAULT_TESTING_CURRENCY);
	let exchange_rate = FixedU128::saturating_from_rational(1u128, 100u128);
	oracle_provider
		.feed_values(vec![(key, exchange_rate)])
		.await
		.expect("Unable to set exchange rate");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_getters() {
	let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Alice).await;

	tokio::join!(
		async {
			assert_eq!(
				parachain_rpc.get_free_balance(DEFAULT_TESTING_CURRENCY).await.unwrap(),
				1 << 60
			);
		},
		async {
			assert_eq!(parachain_rpc.get_parachain_status().await.unwrap(), StatusCode::Error);
		},
		// async {
		// 	assert!(parachain_rpc.get_replace_dust_amount().await.unwrap() > 0);
		// },
		async {
			assert!(parachain_rpc.get_current_active_block_number().await.unwrap() == 0);
		}
	);
}

// These tests don't work for now because the submission does not return a proper error
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn test_invalid_tx_matching() {
	let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Alice).await;
	let err = parachain_rpc.get_invalid_tx_error(AccountKeyring::Bob.into()).await;
	assert!(err.is_invalid_transaction().is_some())
}

#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn test_too_low_priority_matching() {
	let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Alice).await;
	let err = parachain_rpc.get_too_low_priority_error(AccountKeyring::Bob.into()).await;
	assert!(err.is_pool_too_low_priority().is_some())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_subxt_processing_events_after_dispatch_error() {
	let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Alice).await;

	let oracle_provider = setup_provider(client.clone(), AccountKeyring::Bob).await;
	let invalid_oracle = setup_provider(client, AccountKeyring::Dave).await;

	let event_listener = crate::integration::assert_event::<FeedValuesEvent, _>(
		Duration::from_secs(80),
		parachain_rpc.clone(),
		|_| true,
	);

	let key = OracleKey::ExchangeRate(DEFAULT_TESTING_CURRENCY);
	let exchange_rate = FixedU128::saturating_from_rational(1u128, 100u128);

	let result = tokio::join!(
		event_listener,
		invalid_oracle.feed_values(vec![(key.clone(), exchange_rate)]),
		oracle_provider.feed_values(vec![(key, exchange_rate)])
	);

	// ensure first set_exchange_rate failed and second succeeded.
	result.1.unwrap_err();
	result.2.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_register_vault() {
	let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Alice).await;
	set_exchange_rate(client.clone()).await;

	let vault_id = VaultId::new(
		AccountKeyring::Alice.into(),
		DEFAULT_TESTING_CURRENCY,
		DEFAULT_WRAPPED_CURRENCY,
	);

	parachain_rpc.register_public_key(dummy_public_key()).await.unwrap();
	parachain_rpc
		.register_vault(&vault_id, 3 * ForeignCurrencyId::KSM.one())
		.await
		.unwrap();
	parachain_rpc.get_vault(&vault_id).await.unwrap();
	assert_eq!(parachain_rpc.get_public_key().await.unwrap(), Some(dummy_public_key()));
}
