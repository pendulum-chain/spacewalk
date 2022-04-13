use crate::{
    error::Error,
    horizon::{BalanceConversion, CurrencyConversion, CurrencyId, HorizonAccountResponse, StringCurrencyConversion},
};
use runtime::{Balance, RedeemEvent, SpacewalkParachain};
use service::{spawn_cancelable, Error as ServiceError, ShutdownSender};
use sp_runtime::traits::{Convert, StaticLookup};
use sp_std::{
    convert::{From, TryInto},
    prelude::*,
    str,
};
use stellar::{network::TEST_NETWORK, PublicKey, SecretKey};
use substrate_stellar_sdk as stellar;

const SUBMISSION_TIMEOUT_PERIOD: u64 = 10000;

fn is_escrow(escrow_key: &String, public_key: [u8; 32]) -> bool {
    let escrow_keypair: SecretKey = SecretKey::from_encoding(escrow_key).unwrap();
    return public_key == *escrow_keypair.get_public().as_binary();
}

async fn fetch_from_remote(request_url: &str) -> Result<HorizonAccountResponse, Error> {
    tracing::info!("Sending request to: {}", request_url);

    let response = reqwest::get(request_url)
        .await
        .map_err(|_| Error::HttpFetchingError)?
        .json::<HorizonAccountResponse>()
        .await
        .map_err(|_| Error::HttpFetchingError)?;

    Ok(response)
}

async fn fetch_latest_seq_no(stellar_addr: &str) -> Result<u64, Error> {
    let request_url = String::from("https://horizon-testnet.stellar.org/accounts/") + stellar_addr;

    let horizon_response = fetch_from_remote(request_url.as_str()).await.map_err(|e| {
        tracing::error!("fetch_latest_seq_no error: {:?}", e);
        Error::HttpFetchingError
    })?;

    String::from_utf8(horizon_response.sequence)
        .map(|string| string.parse::<u64>().unwrap())
        .map_err(|_| Error::SeqNoParsingError)
}

fn sign_stellar_tx(
    tx: stellar::types::Transaction,
    secret_key: SecretKey,
) -> Result<stellar::TransactionEnvelope, Error> {
    let mut envelope = tx.into_transaction_envelope();
    envelope.sign(&TEST_NETWORK, vec![&secret_key])?;

    Ok(envelope)
}

fn submit_stellar_tx(tx: stellar::TransactionEnvelope) -> Result<(), Error> {
    let mut last_error: Option<Error> = None;

    for attempt in 1..=3 {
        tracing::debug!("Attempt #{} to submit Stellar transaction…", attempt);

        match try_once_submit_stellar_tx(&tx) {
            Ok(result) => {
                return Ok(result);
            }
            Err(error) => {
                last_error = Some(error);
            }
        }
    }

    // Can only panic if no submission was ever attempted
    Err(last_error.unwrap())
}

fn try_once_submit_stellar_tx(tx: &stellar::TransactionEnvelope) -> Result<(), Error> {
    let horizon_base_url = "https://horizon-testnet.stellar.org";
    let horizon = stellar::horizon::Horizon::new(horizon_base_url);

    tracing::info!("Submitting transaction to Stellar network: {}", horizon_base_url);

    let _response = horizon
        .submit_transaction(&tx, SUBMISSION_TIMEOUT_PERIOD, true)
        .map_err(|error| {
            match error {
                stellar::horizon::FetchError::UnexpectedResponseStatus { status, body } => {
                    tracing::error!("Unexpected HTTP request status code: {}", status);
                    tracing::error!("Response body: {}", str::from_utf8(&body).unwrap());
                }
                _ => (),
            }
            Error::HttpFetchingError
        })?;

    Ok(())
}

fn create_withdrawal_tx(
    escrow_secret_key: &String,
    stellar_addr: &stellar::PublicKey,
    seq_num: i64,
    asset: stellar::Asset,
    amount: Balance,
) -> Result<stellar::Transaction, Error> {
    let destination_addr = stellar_addr.as_binary();

    let source_keypair: SecretKey = SecretKey::from_encoding(escrow_secret_key).unwrap();

    let source_pubkey = source_keypair.get_public().clone();

    let mut tx = stellar::Transaction::new(source_pubkey, seq_num, Some(10_000), None, None)?;

    tx.append_operation(stellar::Operation::new_payment(
        stellar::MuxedAccount::KeyTypeEd25519(*destination_addr),
        asset,
        stellar::StroopAmount(BalanceConversion::lookup(amount).map_err(|_| Error::BalanceConversionError)?),
    )?)?;

    Ok(tx)
}

async fn execute_withdrawal(
    escrow_secret_key: &String,
    amount: Balance,
    currency_id: CurrencyId,
    destination_stellar_address: PublicKey,
) -> Result<(), Error> {
    let asset = CurrencyConversion::lookup(currency_id)?;

    let escrow_keypair: SecretKey = SecretKey::from_encoding(escrow_secret_key).unwrap();
    let escrow_encoded = escrow_keypair.get_public().to_encoding().clone();
    let escrow_address = str::from_utf8(escrow_encoded.as_slice())?;

    tracing::info!("Execute withdrawal: ({:?}, {:?})", currency_id, amount,);

    let seq_no = fetch_latest_seq_no(escrow_address).await.map(|seq_no| seq_no + 1)?;
    let transaction = create_withdrawal_tx(
        escrow_secret_key,
        &destination_stellar_address,
        seq_no as i64,
        asset,
        amount,
    )?;
    let signed_envelope = sign_stellar_tx(transaction, escrow_keypair)?;

    let result = submit_stellar_tx(signed_envelope);
    tracing::info!(
        "✔️  Successfully submitted withdrawal transaction to Stellar, crediting {}",
        str::from_utf8(destination_stellar_address.to_encoding().as_slice()).unwrap()
    );

    result
}

pub async fn listen_for_redeem_requests(
    shutdown_tx: ShutdownSender,
    parachain_rpc: SpacewalkParachain,
    escrow_secret_key: String,
) -> Result<(), ServiceError> {
    tracing::info!("Starting to listen for redeem requests…");
    parachain_rpc
        .on_event::<RedeemEvent, _, _, _>(
            |event| async {
                tracing::info!("Received redeem request: {:?}", event);

                // within this event callback, we captured the arguments of listen_for_redeem_requests
                // by reference. Since spawn requires static lifetimes, we will need to capture the
                // arguments by value rather than by reference, so clone these:
                let secret_key = escrow_secret_key.clone();
                let asset_code = event.asset_code.clone();
                let asset_issuer = event.asset_issuer.clone();
                let stellar_user_id = event.stellar_user_id.clone();
                let stellar_vault_id = event.stellar_vault_id.clone();
                let amount = event.amount.clone();

                if !is_escrow(&escrow_secret_key, stellar_vault_id.try_into().unwrap()) {
                    tracing::info!("Rejecting redeem request: not an escrow");
                    return;
                }

                // Spawn a new task so that we handle these events concurrently
                spawn_cancelable(shutdown_tx.subscribe(), async move {
                    tracing::info!("Executing redeem #{:?}", event);

                    let currency_id = StringCurrencyConversion::convert((asset_code, asset_issuer)).unwrap();
                    let destination_stellar_address = stellar::PublicKey::from_encoding(stellar_user_id).unwrap();

                    let result =
                        execute_withdrawal(&secret_key, amount, currency_id, destination_stellar_address).await;

                    match result {
                        Ok(_) => tracing::info!(
                            "Completed redeem request with amount {}",
                            // event.redeem_id,
                            event.amount
                        ),
                        Err(e) => tracing::error!(
                            "Failed to process redeem request: {}",
                            // event.redeem_id,
                            e.to_string()
                        ),
                    }
                });
            },
            |error| tracing::error!("Error reading redeem event: {}", error.to_string()),
        )
        .await?;
    Ok(())
}
