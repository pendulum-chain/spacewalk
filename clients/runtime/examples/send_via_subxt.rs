use std::str::FromStr;
use jsonrpsee::async_client::ClientBuilder;
use sp_keyring::AccountKeyring;
use sp_runtime::generic::Era;
use subxt::ext::sp_core::crypto::SecretUri;
use runtime::{metadata, SpacewalkRuntime, SpacewalkSigner};
use subxt::ext::sp_core::sr25519::Pair as SR25519Pair;
use sp_runtime::app_crypto::sp_core;
use subxt::ext::sp_core::Pair;
use subxt::ext::sp_runtime::MultiAddress;
use subxt::tx::TxPayload;


use primitives::CurrencyId;
use runtime::params::{ChargeAssetTxPayment, SpacewalkAdditionalParams, SpacewalkExtraParams, SpacewalkExtrinsicParams};
use subxt_client::SubxtClient;

#[tokio::main]
async fn main() {
    let api = subxt::OnlineClient::<SpacewalkRuntime>::new().await.expect("should work");

    let dest =  SR25519Pair::from_string("//Bob", None).unwrap();
    let dest_pub = subxt::ext::sp_runtime::MultiAddress::Id(AccountKeyring::Bob.into());

    // Build a balance transfer extrinsic.
    let balance_transfer_tx = metadata::tx().balances().transfer(
        dest_pub,
        10_000,
    );

    let alice_pair = SR25519Pair::from_string("//Alice", None).unwrap();
    let from = SpacewalkSigner::new(alice_pair);

    let charge = ChargeAssetTxPayment { tip: 10, asset_id: Some(CurrencyId::XCM(0)) };
    let extra_params = (
        api.genesis_hash(),
        Era::Immortal,
        charge
        );

    let events = api.tx()
        .sign_and_submit_then_watch(&balance_transfer_tx, &from, extra_params)
        .await.expect("should work!")
        .wait_for_finalized_success()
        .await.expect("should work!!!");

    let transfer_event = events.find_first::<metadata::balances::events::Transfer>();

    if let Ok(event) = transfer_event {
        println!("Balance transfer success: {event:?}");
    }








}