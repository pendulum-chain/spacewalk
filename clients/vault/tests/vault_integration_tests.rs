use frame_support::assert_ok;
use futures::{Future, FutureExt};
use runtime::{integration::*, types::*, SpacewalkPallet, SpacewalkParachain, UtilFuncs};
use sp_keyring::Ed25519Keyring as AccountKeyring;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(90);

const STELLAR_VAULT_SECRET_KEY: &str = "SB6WHKIU2HGVBRNKNOEOQUY4GFC4ZLG5XPGWLEAHTIZXBXXYACC76VSQ";

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
    let tx_string: &str = r#"{
  "_links": { },
  "_embedded": {
    "records": [
      {
        "_links": { },
        "id": "0eb09b01df05990221197c67b5c7a03157008e270a690b31f3809b5083506895",
        "paging_token": "876173332480",
        "successful": true,
        "hash": "b9d0b2292c4e09e8eb22d036171491e87b8d2086bf8b265874c8d182cb9c9020",
        "ledger": 176,
        "created_at": "2022-03-16T10:02:39Z",
        "source_account": "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
        "source_account_sequence": "1",
        "fee_account": "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
        "fee_charged": "1100",
        "max_fee": "1100",
        "operation_count": 11,
        "envelope_xdr": "AAAAAgAAAABi/B0L0JGythwN1lY0aypo19NHxvLCyO5tBEcCVvwF9wAABEwAAAAAAAAAAQAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAsAAAAAAAAAAAAAAAAQfdFrLDgzSIIugR73qs8U0ZiKbwBUclTTPh5thlbgnAFjRXhdigAAAAAAAAAAAAAAAAAA3b5KF6uk1w1fSKYLrzR8gF2lB+AHAi6oU6CaWhunAskAAAAXSHboAAAAAAAAAAAAAAAAAHfmNeMLin2aTUfxa530ZRn4zwRu7ROAQfUJeJco8HSCAAHGv1JjQAAAAAAAAAAAAAAAAAAAlRt2go9sp7E1a5ZWvr7vin4UPrFQThpQax1lOFm33AAAABdIdugAAAAAAAAAAAAAAAAAmv+knlR6JR2VqWeU0k/4FgvZ/tSV5DEY4gu0iOTKgpUAAAAXSHboAAAAAAAAAAAAAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAAF0h26AAAAAABAAAAAACVG3aCj2ynsTVrlla+vu+KfhQ+sVBOGlBrHWU4WbfcAAAABgAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bf/////////8AAAABAAAAAJr/pJ5UeiUdlalnlNJP+BYL2f7UleQxGOILtIjkyoKVAAAABgAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bf/////////8AAAABAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAAAQAAAAAAlRt2go9sp7E1a5ZWvr7vin4UPrFQThpQax1lOFm33AAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAJGE5yoAAAAAABAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAAAQAAAACa/6SeVHolHZWpZ5TST/gWC9n+1JXkMRjiC7SI5MqClQAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAJGE5yoAAAAAAAAAAAAAAAAABKBB+2UBMP/abwcm/M1TXO+/JQWhPwkalgqizKmXyRIQx7qh6aAFYAAAAAAAAAAARW/AX3AAAAQDVB8fT2ZXF0PZqtZX9brK0kz+P4G8VKs1DkDklP6ULsvXRexXFBdH4xG8xRAsR1HJeEBH278hiBNNvUwNw6zgzGYc0bAAAAQLgZUU/oYGL7frWDQhJHhCQu9JmfqN03PrJq4/cJrN1OSUWXnmLc94sv8m2L+cxl2p0skr2Jxy+vt1Lcxkv7wAI4WbfcAAAAQHvZEVqlygIProf3jVTZohDWm2WUNrFAFXf1LctTqDCQBHph14Eo+APwrTURLLYTIvNoXeGzBKbL03SsOARWcQLkyoKVAAAAQHAvKv2/Ro4+cNh6bKQO/G9NNiUozYysGwG1GvJQkFjwy/OTsL6WBfuI0Oye84lVBVrQVk2EY1ERFhgdMpuFSg4=",
        "result_xdr": "AAAAAAAABEwAAAAAAAAACwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGAAAAAAAAAAAAAAAGAAAAAAAAAAAAAAABAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        "result_meta_xdr": "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",
        "fee_meta_xdr": "AAAAAgAAAAMAAAABAAAAAAAAAABi/B0L0JGythwN1lY0aypo19NHxvLCyO5tBEcCVvwF9w3gtrOnZAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAEAAACwAAAAAAAAAABi/B0L0JGythwN1lY0aypo19NHxvLCyO5tBEcCVvwF9w3gtrOnY/u0AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAA==",
        "memo_type": "none",
        "signatures": [
          "NUHx9PZlcXQ9mq1lf1usrSTP4/gbxUqzUOQOSU/pQuy9dF7FcUF0fjEbzFECxHUcl4QEfbvyGIE029TA3DrODA==",
          "uBlRT+hgYvt+tYNCEkeEJC70mZ+o3Tc+smrj9wms3U5JRZeeYtz3iy/ybYv5zGXanSySvYnHL6+3UtzGS/vAAg==",
          "e9kRWqXKAg+uh/eNVNmiENabZZQ2sUAVd/Uty1OoMJAEemHXgSj4A/CtNREsthMi82hd4bMEpsvTdKw4BFZxAg==",
          "cC8q/b9Gjj5w2HpspA78b002JSjNjKwbAbUa8lCQWPDL85OwvpYF+4jQ7J7ziVUFWtBWTYRjUREWGB0ym4VKDg=="
        ],
        "valid_after": "1970-01-01T00:00:00Z"
      }
    ]
  }
}"#;

    let transaction: vault::service::HorizonTransactionsResponse = serde_json::from_str(tx_string).unwrap();
    let tx_xdr = &transaction._embedded.records[0].envelope_xdr;

    test_with_vault(|client, vault_provider| async move {
        let deposit_listener = vault::service::poll_horizon_for_new_transactions(
            vault_provider.clone(),
            STELLAR_VAULT_SECRET_KEY.to_string(),
        );

        assert_ok!(vault_provider.report_stellar_transaction(tx_xdr).await);

        test_service(deposit_listener.map(Result::unwrap), async {}).await;
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redeem() {
    test_with_vault(|client, vault_provider| async move {
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);

        // redeem handling
        let redeem_listener = vault::service::listen_for_redeem_requests(
            shutdown_tx,
            vault_provider.clone(),
            STELLAR_VAULT_SECRET_KEY.to_string(),
        );

        test_service(redeem_listener.map(Result::unwrap), async {
            let asset_code = "USDC".as_bytes();
            let asset_issuer = "GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC".as_bytes();
            let amount: u128 = 100000000;

            let vault_keypair = substrate_stellar_sdk::SecretKey::from_encoding(STELLAR_VAULT_SECRET_KEY).unwrap();
            let stellar_vault_pubkey = *vault_keypair.get_public().as_binary();

            tracing::info!("hex key 0x{}", hex::encode(stellar_vault_pubkey));

            // Execute a redeem
            assert_ok!(
                vault_provider
                    .redeem(
                        &asset_code.to_vec(),
                        &asset_issuer.to_vec(),
                        amount,
                        stellar_vault_pubkey.clone()
                    )
                    .await,
            );

            // Expect that a RedeemEvent is registered by the vault provider
            let event = assert_event::<RedeemEvent, _>(TIMEOUT, vault_provider, |_| true).await;
            assert_eq!(event.asset_code, asset_code);
            assert_eq!(event.asset_issuer, asset_issuer);
            assert_eq!(event.stellar_vault_id, stellar_vault_pubkey);
            assert_eq!(event.amount, amount);
        })
        .await;
    })
    .await;
}
