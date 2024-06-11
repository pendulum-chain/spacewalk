#![cfg(test)]

use sp_keyring::AccountKeyring;

use oracle::dia::DiaOracleKeyConvertor;
use primitives::StellarPublicKeyRaw;

use crate::{integration::*, VaultId};

use super::{
	CollateralBalancesPallet, CurrencyId, FixedPointNumber, FixedU128, OraclePallet,
	SecurityPallet, StatusCode, VaultRegistryPallet,
};
use sp_runtime::traits::Convert;

use subxt::utils::AccountId32 as AccountId;
//use std::sync::Arc;

const DEFAULT_TESTING_CURRENCY: CurrencyId = CurrencyId::XCM(0);
const DEFAULT_WRAPPED_CURRENCY: CurrencyId = CurrencyId::AlphaNum4(
	*b"USDC",
	[
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
);

fn dummy_public_key() -> StellarPublicKeyRaw {
	[0u8; 32]
}

async fn set_exchange_rate(client: SubxtClient) {
	let oracle_provider = setup_provider(client, AccountKeyring::Bob).await;
	let key = primitives::oracle::Key::ExchangeRate(DEFAULT_TESTING_CURRENCY);
	let converted_key = DiaOracleKeyConvertor::<MockValue>::convert(key.clone()).unwrap();
	let exchange_rate = FixedU128::saturating_from_rational(1u128, 100u128);
	oracle_provider
		.feed_values(vec![(converted_key, exchange_rate)])
		.await
		.expect("Unable to set exchange rate");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_getters() {
	let is_public_network = false;
	let (client, _tmp_dir) =
		default_provider_client(AccountKeyring::Alice, is_public_network).await;
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
		async {
			assert!(parachain_rpc.get_current_active_block_number().await.unwrap() == 0);
		}
	);
}

// These tests don't work for now because the submission does not return a proper error
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn test_invalid_tx_matching() {
	let is_public_network = false;
	let (client, _tmp_dir) =
		default_provider_client(AccountKeyring::Alice, is_public_network).await;
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Alice).await;
	let err = parachain_rpc.get_invalid_tx_error(AccountId(AccountKeyring::Bob.to_account_id().clone().into())).await;
	assert!(err.is_invalid_transaction().is_some())
}

#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn test_too_low_priority_matching() {
	let is_public_network = false;
	let (client, _tmp_dir) =
		default_provider_client(AccountKeyring::Alice, is_public_network).await;
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Alice).await;
	let err = parachain_rpc.get_too_low_priority_error(AccountId(AccountKeyring::Bob.to_account_id().clone().into())).await;
	assert!(err.is_pool_too_low_priority())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_subxt_processing_events_after_dispatch_error() {
	env_logger::init_from_env(
        env_logger::Env::default()
            .filter_or(env_logger::DEFAULT_FILTER_ENV, log::LevelFilter::Info.as_str()),
    );
	let is_public_network = false;
	let (client, _tmp_dir) =
		default_provider_client(AccountKeyring::Alice, is_public_network).await;

	let oracle_provider = setup_provider(client.clone(), AccountKeyring::Bob).await;
	let invalid_oracle = setup_provider(client, AccountKeyring::Dave).await;

	// let oracle_provider = Arc::new(oracle_provider);
    // let oracle_provider_seal_rpc = Arc::clone(&oracle_provider);

	// let invalid_oracle = Arc::new(invalid_oracle);
    // let invalid_oracle_seal_rpc = Arc::clone(&invalid_oracle);

	// //This process will also stop once the test is finalized
    // tokio::spawn(async move {
    //     loop {
    //         println!("Manually finalizing block");
	// 		invalid_oracle_seal_rpc.manual_seal().await;
	// 		oracle_provider_seal_rpc.manual_seal().await;
	// 		tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    //     }
    // });
	tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

	let key = primitives::oracle::Key::ExchangeRate(DEFAULT_TESTING_CURRENCY);
	let converted_key = DiaOracleKeyConvertor::<MockValue>::convert(key.clone()).unwrap();
	let exchange_rate = FixedU128::saturating_from_rational(1u128, 100u128);

	let result = tokio::join!(
		invalid_oracle.feed_values(vec![(converted_key.clone(), exchange_rate)]),
		oracle_provider.feed_values(vec![(converted_key, exchange_rate)])
	);

	// ensure first set_exchange_rate failed and second succeeded.
	result.0.unwrap_err();
	result.1.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_register_vault() {
    env_logger::init_from_env(
        env_logger::Env::default()
            .filter_or(env_logger::DEFAULT_FILTER_ENV, log::LevelFilter::Info.as_str()),
    );
    let is_public_network = false;
    let (client, _tmp_dir) =
        default_provider_client(AccountKeyring::Alice, is_public_network).await;
    let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Alice).await;
 
    // let parachain_rpc = Arc::new(parachain_rpc);
    // let seal_rpc = Arc::clone(&parachain_rpc);


	//This process will also stop once the test is finalized
    // tokio::spawn(async move {
    //     loop {
           
	// 		tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;
	// 		println!("Manually finalizing block");
	// 		seal_rpc.manual_finalize().await;
			
    //     }
    // });

	// It is completely random depending on the waiting time. 
	tokio::time::sleep(tokio::time::Duration::from_secs(9)).await;
    set_exchange_rate(client.clone()).await;

    let vault_id = VaultId::new(
        AccountId(AccountKeyring::Alice.to_account_id().clone().into()),
        DEFAULT_TESTING_CURRENCY,
        DEFAULT_WRAPPED_CURRENCY,
    );
	tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    println!("Register pk");
    parachain_rpc.register_public_key(dummy_public_key()).await.unwrap();
	tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    println!("Register vault");
    parachain_rpc.register_vault(&vault_id, 3 * 10u128.pow(12)).await.unwrap();
	tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    println!("Getting vault");
    parachain_rpc.get_vault(&vault_id).await.unwrap();
    assert_eq!(parachain_rpc.get_public_key().await.unwrap(), Some(dummy_public_key()));
}
