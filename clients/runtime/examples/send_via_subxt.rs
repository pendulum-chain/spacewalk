use sp_runtime::generic::Era;
use runtime::{metadata, SpacewalkRuntime};
use subxt_signer::sr25519::dev;
use runtime::params::{ChargeAssetTxPayment, SpacewalkAdditionalParams, SpacewalkExtraParams, SpacewalkExtrinsicParams};

#[tokio::main]
async fn main() {
    let api = subxt::OnlineClient::<SpacewalkRuntime>::new().await?;


    // Build a balance transfer extrinsic.
    let dest = dev::bob().public_key().into();
    let balance_transfer_tx = metadata::tx().balances().transfer(dest, 10_000);

    let from = dev::alice();

    let events = api.tx()
        .sign_and_submit_then_watch(&balance_transfer_tx, &from,
        SpacewalkExtrinsicParams {
            extra_params: SpacewalkExtraParams {
                era: Era::Immortal,
                nonce: 0,
                charge: ChargeAssetTxPayment { tip: 10, asset_id: None },
            },
            additional_params: SpacewalkAdditionalParams,
        }
        )
        .await?
        .wait_for_finalized_success()
        .await?;

    let transfer_event = events.find_first::<metadata::balances::events::Transfer>()?;

    if let Some(event) = transfer_event {
        println!("Balance transfer success: {event:?}");
    }








}