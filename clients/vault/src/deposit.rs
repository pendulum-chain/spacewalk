use crate::{
    error::Error,
    horizon::{HorizonTransactionsResponse, Transaction},
};
use runtime::SpacewalkPallet;
use service::Error as ServiceError;
use sp_std::{convert::From, str, vec::Vec};
use std::time::Duration;
use stellar::SecretKey;
use substrate_stellar_sdk as stellar;
use tokio::time::sleep;

const POLL_INTERVAL: u64 = 5000;

/// Fetch recent transactions from remote and deserialize to HorizonResponse
/// Since the limit in the request url is set to one it will always fetch just one
async fn fetch_latest_txs(escrow_secret_key: &String) -> Result<HorizonTransactionsResponse, Error> {
    let escrow_keypair: SecretKey = SecretKey::from_encoding(escrow_secret_key).unwrap();
    let escrow_address = escrow_keypair.get_public();

    let request_url = String::from("https://horizon-testnet.stellar.org/accounts/")
        + str::from_utf8(escrow_address.to_encoding().as_slice()).map_err(|_| Error::HttpFetchingError)?
        + "/transactions?order=desc&limit=1";

    let horizon_response = reqwest::get(request_url.as_str())
        .await
        .map_err(|_| Error::HttpFetchingError)?
        .json::<HorizonTransactionsResponse>()
        .await
        .map_err(|_| Error::HttpFetchingError)?;

    Ok(horizon_response)
}

static mut LAST_TX_ID: Option<Vec<u8>> = None;

fn is_unhandled_transaction(tx: &Transaction) -> bool {
    const UP_TO_DATE: () = ();
    let latest_tx_id_utf8 = &tx.id;

    unsafe {
        let prev_tx_id = &LAST_TX_ID;
        let initial = !matches!(prev_tx_id, Some(_));

        let result = match prev_tx_id {
            Some(prev_tx_id) => {
                if prev_tx_id == latest_tx_id_utf8 {
                    Err(UP_TO_DATE)
                } else {
                    Ok(latest_tx_id_utf8.clone())
                }
            }
            None => Ok(latest_tx_id_utf8.clone()),
        };

        match result {
            Ok(latest_tx_id) => {
                LAST_TX_ID = Some(latest_tx_id.clone());
                if !initial {
                    tracing::info!(
                        "Found new transaction from Horizon (id {:#?}). Starting to process new transaction",
                        str::from_utf8(&latest_tx_id).unwrap()
                    );

                    true
                } else {
                    tracing::info!("Initial transaction handled");
                    false
                }
            }
            Err(UP_TO_DATE) => {
                tracing::info!("Already up to date");
                false
            }
        }
    }
}

async fn report_transaction<P: SpacewalkPallet>(parachain_rpc: &P, tx: &Transaction) -> Result<(), Error> {
    // Decode transaction to Base64
    let tx_xdr = base64::decode(&tx.envelope_xdr).unwrap();

    // Send new transaction to spacewalk bridge pallet
    let result = parachain_rpc.report_stellar_transaction(&tx_xdr).await;
    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::RuntimeError(e)),
    }
}

async fn fetch_horizon_and_process_new_transactions<P: SpacewalkPallet>(parachain_rpc: &P, escrow_secret_key: &String) {
    let res = fetch_latest_txs(escrow_secret_key).await;
    let transactions = match res {
        Ok(txs) => txs._embedded.records,
        Err(e) => {
            tracing::warn!("Failed to fetch transactions: {:?}", e);
            return;
        }
    };

    if transactions.len() > 0 {
        let tx = &transactions[0];
        if is_unhandled_transaction(tx) {
            let result = report_transaction(parachain_rpc, tx).await;
            match result {
                Ok(_) => {
                    tracing::info!("Reported Stellar transaction to spacewalk pallet");
                }
                Err(e) => {
                    tracing::warn!("Failed to process transaction: {:?}", e);
                }
            }
        }
    }
}

pub async fn poll_horizon_for_new_transactions<P: SpacewalkPallet>(
    parachain_rpc: P,
    escrow_secret_key: String,
) -> Result<(), ServiceError> {
    // Start polling horizon every 5 seconds
    loop {
        fetch_horizon_and_process_new_transactions(&parachain_rpc, &escrow_secret_key).await;

        sleep(Duration::from_millis(POLL_INTERVAL)).await;
    }
}
