use frame_support::assert_ok;
use futures::{
    channel::mpsc,
    future::{join, join3, join4, join5, try_join},
    Future, FutureExt, SinkExt, TryStreamExt,
};
use runtime::{integration::*, types::*, CurrencyId, FixedPointNumber, FixedU128, SpacewalkParachain, UtilFuncs};
use sp_core::{H160, H256};
use sp_keyring::AccountKeyring;
use std::{sync::Arc, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(90);

const DEFAULT_NATIVE_CURRENCY: CurrencyId = Token(INTR);
const DEFAULT_TESTING_CURRENCY: CurrencyId = Token(DOT);
const DEFAULT_WRAPPED_CURRENCY: CurrencyId = Token(INTERBTC);

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
