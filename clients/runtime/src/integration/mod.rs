#![cfg(all(feature = "testing-utils", feature = "standalone-metadata"))]

use crate::{InterBtcSigner, SpacewalkParachain};
use subxt_client::{AccountKeyring, DatabaseSource, KeystoreConfig, Role, SubxtClientConfig, WasmExecutionMethod};
use tempdir::TempDir;

pub use subxt_client::SubxtClient;

/// Start a new instance of the parachain. The second item in the returned tuple must remain in
/// scope as long as the parachain is active, since dropping it will remove the temporary directory
/// that the parachain uses
pub async fn default_provider_client(key: AccountKeyring) -> (SubxtClient, TempDir) {
    let tmp = TempDir::new("btc-parachain-").expect("failed to create tempdir");
    let config = SubxtClientConfig {
        impl_name: "btc-parachain-full-client",
        impl_version: "0.0.1",
        author: "Interlay Ltd",
        copyright_start_year: 2020,
        db: DatabaseSource::ParityDb {
            path: tmp.path().join("db"),
        },
        keystore: KeystoreConfig::Path {
            path: tmp.path().join("keystore"),
            password: None,
        },
        chain_spec: interbtc::chain_spec::development_config(),
        role: Role::Authority(key),
        telemetry: None,
        wasm_method: WasmExecutionMethod::Compiled,
        tokio_handle: tokio::runtime::Handle::current(),
    };

    // enable off chain workers
    let mut service_config = config.into_service_config();
    service_config.offchain_worker.enabled = true;

    let (task_manager, rpc_handlers) = interbtc::service::new_full(service_config).unwrap();

    let client = SubxtClient::new(task_manager, rpc_handlers);

    let root_provider = setup_provider(client.clone(), AccountKeyring::Alice).await;

    (client, tmp)
}

/// Create a new parachain_rpc with the given keyring
pub async fn setup_provider(client: SubxtClient, key: AccountKeyring) -> SpacewalkParachain {
    let signer = InterBtcSigner::new(key.pair());
    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<u128>(16);

    SpacewalkParachain::new(client, signer)
        .await
        .expect("Error creating parachain_rpc")
}
