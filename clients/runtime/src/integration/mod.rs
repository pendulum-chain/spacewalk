#![cfg(feature = "testing-utils")]

use std::{sync::Arc, time::Duration};
use std::process::{Child, Command};

use frame_support::assert_ok;
use futures::{future::Either, pin_mut, Future, FutureExt, SinkExt, StreamExt};
use sp_keyring::AccountKeyring;
use oracle::dia::{DiaOracleKeyConvertor, NativeCurrencyKey, XCMCurrencyConversion};
use sp_runtime::traits::Convert;
use subxt::{
	events::StaticEvent as Event,
	ext::sp_core::{sr25519::Pair, Pair as _},
};
use tempdir::TempDir;
use tokio::{sync::RwLock, time::timeout};


use crate::{rpc::{OraclePallet, VaultRegistryPallet}, CurrencyId, FixedU128, SpacewalkParachain, SpacewalkSigner, SudoPallet};
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

fn get_pair_from_keyring(keyring: AccountKeyring) -> Result<Pair, String> {
	let pair = Pair::from_string(keyring.to_seed().as_str(), None)
		.map_err(|_| "Could not create pair for keyring")?;
	Ok(pair)
}

/// Start a new instance of the parachain. The second item in the returned tuple must remain in
/// scope as long as the parachain is active, since dropping it will remove the temporary directory
/// that the parachain uses
pub async fn default_root_provider(key: AccountKeyring) -> (SpacewalkParachain, TempDir) {
	let tmp = TempDir::new("spacewalk-parachain-").expect("failed to create tempdir");
	let root_provider = setup_provider(key).await;

	if let Err(e) = root_provider.set_parachain_confirmations(1).await {
		log::warn!("default_root_provider(): ERROR: {e:?}");
	}

	(root_provider, tmp)
}

/// Create a new parachain_rpc with the given keyring
pub async fn setup_provider(key: AccountKeyring) -> SpacewalkParachain {
	let pair = get_pair_from_keyring(key).unwrap();
	let signer = SpacewalkSigner::new(pair);
	let signer = Arc::new(RwLock::new(signer));
	let shutdown_tx = crate::ShutdownSender::new();
	let parachain_rpc = SpacewalkParachain::from_url_with_retry(
		&"ws://127.0.0.1:9944".to_string(),
		signer.clone(),
		Duration::from_secs(20),
		shutdown_tx,
	)
		.await
		.unwrap();
	parachain_rpc
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


pub async fn start_chain() -> std::io::Result<Child> {
	let res = std::env::current_dir();
	println!("HEEEEEEEEEEEEY: {res:?}");
	let command = Command::new("sh").arg("./scripts/run_parachain_node.sh").spawn();
	command
}