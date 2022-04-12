use frame_support::assert_ok;
use futures::{future::join, Future, FutureExt};
use runtime::{integration::*, types::*, CurrencyId, SpacewalkParachain, UtilFuncs};
use sp_keyring::AccountKeyring;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(90);

const DEFAULT_NATIVE_CURRENCY: CurrencyId = Token(INTR);
const DEFAULT_TESTING_CURRENCY: CurrencyId = Token(DOT);
const DEFAULT_WRAPPED_CURRENCY: CurrencyId = Token(INTERBTC);

const STELLAR_ESCROW_SECRET_KEY: &str = "SA4OOLVVZV2W7XAKFXUEKLMQ6Y2W5JBENHO5LP6W6BCPBU3WUZ5EBT7K";

async fn test_with<F, R>(execute: impl FnOnce(SubxtClient) -> F) -> R
where
    F: Future<Output = R>,
{
    service::init_subscriber();
    let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;

    let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;

    execute(client).await
}

async fn test_with_vault<F, R>(execute: impl FnOnce(SubxtClient, SpacewalkParachain) -> F) -> R
where
    F: Future<Output = R>,
{
    service::init_subscriber();
    let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;

    let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;

    let vault_provider = setup_provider(client.clone(), AccountKeyring::Charlie).await;

    execute(client, vault_provider).await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_deposit() {
    test_with_vault(|client, vault_provider| async move {
        let deposit_listener = vault::service::poll_horizon_for_new_transactions(
            vault_provider.clone(),
            STELLAR_ESCROW_SECRET_KEY.to_string(),
        );

        test_service(deposit_listener.map(Result::unwrap), async {}).await;
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redeem() {
    test_with_vault(|client, vault_provider| async move {
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);

        let deposit_listener = vault::service::poll_horizon_for_new_transactions(
            vault_provider.clone(),
            STELLAR_ESCROW_SECRET_KEY.to_string(),
        );

        // redeem handling
        let redeem_listener = vault::service::listen_for_redeem_requests(
            shutdown_tx,
            vault_provider.clone(),
            STELLAR_ESCROW_SECRET_KEY.to_string(),
        );

        test_service(
            join(
                deposit_listener.map(Result::unwrap),
                redeem_listener.map(Result::unwrap),
            ),
            async {},
        )
        .await;
    })
    .await;
}
