#![cfg(feature = "testing-utils")]

use std::{sync::Arc, time::Duration};

use frame_support::assert_ok;
use futures::{future::Either, pin_mut, Future, FutureExt, SinkExt, StreamExt};
use oracle::dia::{DiaOracleKeyConvertor, NativeCurrencyKey, XCMCurrencyConversion};
use sp_runtime::traits::Convert;
use subxt::{
	events::StaticEvent as Event,
	ext::sp_core::{sr25519::Pair, Pair as _},
};
use tempdir::TempDir;
use tokio::{sync::RwLock, time::timeout};

pub use subxt_client::SubxtClient;
use subxt_client::{
	AccountKeyring, DatabaseSource, KeystoreConfig, Role, SubxtClientConfig, WasmExecutionMethod,
	WasmtimeInstantiationStrategy,
};

use crate::{
	rpc::{OraclePallet, VaultRegistryPallet},
	CurrencyId, FixedU128, SpacewalkParachain, SpacewalkSigner,
};
use primitives::oracle::Key as OracleKey;

pub struct MockValue;

impl NativeCurrencyKey for MockValue {
	fn native_symbol() -> Vec<u8> {
		"NativeKey".as_bytes().to_vec()
	}

	fn native_chain() -> Vec<u8> {
		"TestChain".as_bytes().to_vec()
	}
}

impl XCMCurrencyConversion for MockValue {
	fn convert_to_dia_currency_id(token_symbol: u8) -> Option<(Vec<u8>, Vec<u8>)> {
		// We assume that the blockchain is always 0 and the symbol represents the token symbol
		let blockchain = vec![0u8];
		let symbol = vec![token_symbol];
		Some((blockchain, symbol))
	}

	fn convert_from_dia_currency_id(blockchain: Vec<u8>, symbol: Vec<u8>) -> Option<u8> {
		// We assume that the blockchain is always 0 and the symbol represents the token symbol
		if blockchain.len() != 1 && blockchain[0] != 0 || symbol.len() != 1 {
			return None
		}
		return Some(symbol[0])
	}
}

/// Start a new instance of the parachain. The second item in the returned tuple must remain in
/// scope as long as the parachain is active, since dropping it will remove the temporary directory
/// that the parachain uses
pub async fn default_provider_client(
	key: AccountKeyring,
	is_public_network: bool,
) -> (SubxtClient, TempDir) {
	let chain_spec = if is_public_network {
		testchain::chain_spec::mainnet_config()
	} else {
		testchain::chain_spec::testnet_config()
	};

	// This does not necessarily have to match the `impl_name` of the runtime. We give it a
	// descriptive name because it will be shown when starting the testing client.
	let impl_name = if is_public_network {
		"spacewalk-standalone-mainnet"
	} else {
		"spacewalk-standalone-testnet"
	};

	let tmp = TempDir::new("spacewalk-parachain-").expect("failed to create tempdir");
	let config = SubxtClientConfig {
		impl_name,
		impl_version: "0.0.1",
		author: "SatoshiPay",
		copyright_start_year: 2020,
		db: DatabaseSource::ParityDb { path: tmp.path().join("db") },
		keystore: KeystoreConfig::Path { path: tmp.path().join("keystore"), password: None },
		chain_spec,
		role: Role::Authority(key),
		telemetry: None,
		wasm_method: WasmExecutionMethod::Compiled {
			instantiation_strategy: WasmtimeInstantiationStrategy::LegacyInstanceReuse,
		},
		tokio_handle: tokio::runtime::Handle::current(),
	};

	// enable off chain workers
	let mut service_config = config.into_service_config();
	service_config.offchain_worker.enabled = true;

	let (task_manager, rpc_handlers) = if is_public_network {
		testchain::service::start_instant_mainnet(service_config)
			.await
			.expect("Couldn't start mainnet")
	} else {
		testchain::service::start_instant_testnet(service_config)
			.await
			.expect("Couldn't start testnet")
	};

	let client = SubxtClient::new(task_manager, rpc_handlers);

	(client, tmp)
}

fn get_pair_from_keyring(keyring: AccountKeyring) -> Result<Pair, String> {
	let pair = Pair::from_string(keyring.to_seed().as_str(), None)
		.map_err(|_| "Could not create pair for keyring")?;
	Ok(pair)
}

/// Create a new parachain_rpc with the given keyring
pub async fn setup_provider(client: SubxtClient, key: AccountKeyring) -> SpacewalkParachain {
	let pair = get_pair_from_keyring(key).unwrap();
	let signer = SpacewalkSigner::new(pair);
	let signer = Arc::new(RwLock::new(signer));
	let shutdown_tx = crate::ShutdownSender::new();

	SpacewalkParachain::new(client.into(), signer, shutdown_tx)
		.await
		.expect("Error creating parachain_rpc")
}

pub async fn set_exchange_rate_and_wait(
	parachain_rpc: &SpacewalkParachain,
	currency_id: CurrencyId,
	value: FixedU128,
) {
	let key = OracleKey::ExchangeRate(currency_id);
	let converted_key = DiaOracleKeyConvertor::<MockValue>::convert(key.clone()).unwrap();
	assert_ok!(parachain_rpc.feed_values(vec![(converted_key, value)]).await);
	parachain_rpc.manual_seal().await;
}

pub async fn get_exchange_rate(parachain_rpc: &SpacewalkParachain, currency_id: CurrencyId) {
	let key = OracleKey::ExchangeRate(currency_id);
	let converted_key = DiaOracleKeyConvertor::<MockValue>::convert(key.clone()).unwrap();
	assert_ok!(parachain_rpc.get_exchange_rate(converted_key.0, converted_key.1).await);
}

/// calculate how much collateral the vault requires to accept an issue of the given size
pub async fn get_required_vault_collateral_for_issue(
	parachain_rpc: &SpacewalkParachain,
	amount: u128,
	wrapped_currency: CurrencyId,
	collateral_currency: CurrencyId,
) -> u128 {
	parachain_rpc
		.get_required_collateral_for_wrapped(amount, wrapped_currency, collateral_currency)
		.await
		.unwrap()
}

/// wait for an event to occur. After the specified error, this will panic. This returns the event.
pub async fn assert_event<T, F>(duration: Duration, parachain_rpc: SpacewalkParachain, f: F) -> T
where
	T: Event + Clone + std::fmt::Debug,
	F: Fn(T) -> bool,
{
	let (tx, mut rx) = futures::channel::mpsc::channel(1);
	let event_writer = parachain_rpc
		.on_event::<T, _, _, _>(
			|event| async {
				if (f)(event.clone()) {
					tx.clone().send(event).await.unwrap();
				}
			},
			|_| {},
		)
		.fuse();
	let event_reader = rx.next().fuse();
	pin_mut!(event_reader, event_writer);

	timeout(duration, async {
		match futures::future::select(event_writer, event_reader).await {
			Either::Right((ret, _)) => ret.unwrap(),
			_ => panic!(),
		}
	})
	.await
	.unwrap_or_else(|_| panic!("could not find event: {}::{}", T::PALLET, T::EVENT))
}

/// run `service` in the background, and run `fut`. If the service completes before the
/// second future, this will panic
pub async fn test_service<T: Future, U: Future>(service: T, fut: U) -> U::Output {
	pin_mut!(service, fut);
	match futures::future::select(service, fut).await {
		Either::Right((ret, _)) => ret,
		_ => panic!(),
	}
}

pub async fn with_timeout<T: Future>(future: T, duration: Duration) -> T::Output {
	timeout(duration, future).await.expect("timeout")
}

pub async fn periodically_produce_blocks(parachain_rpc: SpacewalkParachain) {
	loop {
		tokio::time::sleep(Duration::from_millis(500)).await;
		parachain_rpc.manual_seal().await;
	}
}
