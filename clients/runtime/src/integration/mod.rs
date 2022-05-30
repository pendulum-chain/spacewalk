#![cfg(all(feature = "testing-utils", feature = "standalone-metadata"))]

use crate::{SpacewalkParachain, SpacewalkSigner};
use futures::{future::Either, pin_mut, Future, FutureExt, SinkExt, StreamExt};
use std::time::Duration;
use subxt::Event;
use subxt_client::{AccountKeyring, DatabaseSource, KeystoreConfig, Role, SubxtClientConfig, WasmExecutionMethod};
use tempdir::TempDir;
use tokio::time::timeout;

pub use subxt_client::SubxtClient;

/// Start a new instance of the parachain. The second item in the returned tuple must remain in
/// scope as long as the parachain is active, since dropping it will remove the temporary directory
/// that the parachain uses
pub async fn default_provider_client(key: AccountKeyring) -> (SubxtClient, TempDir) {
    let tmp = TempDir::new("spacewalk-parachain-").expect("failed to create tempdir");
    let config = SubxtClientConfig {
        impl_name: "spacewalk-chain-full-client",
        impl_version: "1",
        author: "SatoshiPay",
        copyright_start_year: 2020,
        db: DatabaseSource::ParityDb {
            path: tmp.path().join("db"),
        },
        keystore: KeystoreConfig::Path {
            path: tmp.path().join("keystore"),
            password: None,
        },
        chain_spec: testchain::chain_spec::development_config(),
        role: Role::Authority(key),
        telemetry: None,
        wasm_method: WasmExecutionMethod::Compiled,
        tokio_handle: tokio::runtime::Handle::current(),
    };

    // enable off chain workers
    let mut service_config = config.into_service_config();
    service_config.offchain_worker.enabled = true;

    let (task_manager, rpc_handlers) = testchain::service::new_full(service_config).unwrap();

    let client = SubxtClient::new(task_manager, rpc_handlers);

    let root_provider = setup_provider(client.clone(), AccountKeyring::Alice).await;

    (client, tmp)
}

/// Create a new parachain_rpc with the given keyring
pub async fn setup_provider(client: SubxtClient, key: AccountKeyring) -> SpacewalkParachain {
    let signer = SpacewalkSigner::new(key.pair());
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);

    SpacewalkParachain::new(client, signer, shutdown_tx)
        .await
        .expect("Error creating parachain_rpc")
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
