use crate::error::Error;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Deserializer};

use sp_core::ed25519;
use sp_runtime::{
    traits::{IdentifyAccount, LookupError, StaticLookup},
    AccountId32, MultiSigner,
};
use sp_std::{convert::From, prelude::*, str};
use stellar::{
    network::TEST_NETWORK,
    types::{OperationBody, PaymentOp},
    IntoAmount, SecretKey, StellarSdkError, XdrCodec,
};
use substrate_stellar_sdk as stellar;

// This represents each record for a transaction in the Horizon API response
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct Transaction {
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub id: Vec<u8>,
    successful: bool,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub hash: Vec<u8>,
    ledger: u32,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub created_at: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub source_account: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub source_account_sequence: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub fee_account: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub fee_charged: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub max_fee: Vec<u8>,
    operation_count: u32,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub envelope_xdr: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub result_xdr: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub result_meta_xdr: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub fee_meta_xdr: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub memo_type: Vec<u8>,
}

// The following structs represent the whole response when fetching any Horizon API
// In this particular case we assume the embedded payload will allways be for transactions
// ref https://developers.stellar.org/api/introduction/response-format/
#[derive(Deserialize, Debug)]
pub struct EmbeddedTransactions {
    pub records: Vec<Transaction>,
}

#[derive(Deserialize, Debug)]
pub struct HorizonAccountResponse {
    // We don't care about specifics of pagination, so we just tell serde that this will be a generic json value
    pub _links: serde_json::Value,

    #[serde(deserialize_with = "de_string_to_bytes")]
    pub id: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub account_id: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub sequence: Vec<u8>,
    // ...
}

#[derive(Deserialize, Debug)]
pub struct HorizonTransactionsResponse {
    // We don't care about specifics of pagination, so we just tell serde that this will be a generic json value
    pub _links: serde_json::Value,
    pub _embedded: EmbeddedTransactions,
}

pub fn de_string_to_bytes<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(de)?;
    Ok(s.as_bytes().to_vec())
}

// Claimable balances objects

#[derive(Deserialize, Debug)]
pub struct HorizonClaimableBalanceResponse {
    // We don't care about specifics of pagination, so we just tell serde that this will be a generic json value
    pub _links: serde_json::Value,
    pub _embedded: EmbeddedClaimableBalance,
}

// The following structs represent the whole response when fetching any Horizon API
// for retreiving a list of claimable balances for an account
#[derive(Deserialize, Debug)]
pub struct EmbeddedClaimableBalance {
    pub records: Vec<ClaimableBalance>,
}

// This represents each record for a claimable balance in the Horizon API response
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct ClaimableBalance {
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub id: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub paging_token: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub asset: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub amount: Vec<u8>,
    pub claimants: Vec<Claimant>,
    pub last_modified_ledger: u32,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub last_modified_time: Vec<u8>,
}

// This represents a Claimant
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct Claimant {
    #[serde(deserialize_with = "de_string_to_bytes")]
    pub destination: Vec<u8>,
    // For now we assume that the predicate is always unconditional
    // pub predicate: serde_json::Value,
}

pub struct AddressConversion;

impl StaticLookup for AddressConversion {
    type Source = AccountId32;
    type Target = stellar::PublicKey;

    fn lookup(key: Self::Source) -> Result<Self::Target, LookupError> {
        // We just assume (!) an Ed25519 key has been passed to us
        Ok(stellar::PublicKey::from_binary(key.into()) as stellar::PublicKey)
    }

    fn unlookup(stellar_addr: stellar::PublicKey) -> Self::Source {
        MultiSigner::Ed25519(ed25519::Public::from_raw(*stellar_addr.as_binary())).into_account()
    }
}

/// Error type for key decoding errors
#[derive(Debug)]
pub enum AddressConversionError {
    //     UnexpectedKeyType
}

// Network functions

/// Fetch recent transactions from remote and deserialize to HorizonResponse
pub async fn fetch_latest_txs() -> Result<HorizonTransactionsResponse, Error> {
    // TODO replace with key that is supplied by CLI config
    let escrow_keypair: SecretKey =
        SecretKey::from_encoding("SA4OOLVVZV2W7XAKFXUEKLMQ6Y2W5JBENHO5LP6W6BCPBU3WUZ5EBT7K").unwrap();
    let escrow_address = escrow_keypair.get_public();

    let request_url = String::from("https://horizon-testnet.stellar.org/accounts/")
        + str::from_utf8(escrow_address.to_encoding().as_slice()).map_err(|e| Error::HttpFetchingError)?
        + "/transactions?order=desc&limit=1";

    let horizon_response = reqwest::get(request_url.as_str())
        .await
        .map_err(|e| Error::HttpFetchingError)?
        .json::<HorizonTransactionsResponse>()
        .await
        .map_err(|e| Error::HttpFetchingError)?;

    Ok(horizon_response)
}

static mut LAST_TX_ID: Option<Vec<u8>> = None;

pub fn handle_new_transaction(tx: &Transaction) {
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
                        "✴️  New transaction from Horizon (id {:#?}). Starting to replicate transaction in Pendulum.",
                        str::from_utf8(&latest_tx_id).unwrap()
                    );

                    // Decode transaction to Base64 and then to Stellar XDR to get transaction details
                    let tx_xdr = base64::decode(&tx.envelope_xdr).unwrap();
                    let tx_envelope = stellar::TransactionEnvelope::from_xdr(&tx_xdr).unwrap();

                    if let stellar::TransactionEnvelope::EnvelopeTypeTx(env) = tx_envelope {
                        process_new_transaction(env.tx);
                    }
                } else {
                    tracing::info!("Initial transaction handled");
                }
            }
            Err(UP_TO_DATE) => {
                tracing::info!("Already up to date");
            }
        }
    }
}

pub fn process_new_transaction(transaction: stellar::types::Transaction) {
    // The destination of a mirrored Pendulum transaction, is always derived of the source account that initiated
    // the Stellar transaction.
    tracing::info!("Processing transaction");
    let destination = if let stellar::MuxedAccount::KeyTypeEd25519(key) = transaction.source_account {
        AddressConversion::unlookup(stellar::PublicKey::from_binary(key))
    } else {
        tracing::error!("❌  Source account format not supported.");
        return;
    };

    let payment_ops: Vec<&PaymentOp> = transaction
        .operations
        .get_vec()
        .into_iter()
        .filter_map(|op| match &op.body {
            OperationBody::Payment(p) => Some(p),
            _ => None,
        })
        .collect();

    for payment_op in payment_ops {
        let _dest_account = stellar::MuxedAccount::from(payment_op.destination.clone());

        // if let stellar::MuxedAccount::KeyTypeEd25519(payment_dest_public_key) = payment_op.destination {
        //     if Self::is_escrow(payment_dest_public_key) {
        //         let amount = T::BalanceConversion::unlookup(payment_op.amount);
        //         let currency = T::CurrencyConversion::unlookup(payment_op.asset.clone());

        //         match Self::offchain_unsigned_tx_signed_payload(currency, amount, destination) {
        //             Err(_) => tracing::warn!("Sending the tx failed."),
        //             Ok(_) => {
        //                 tracing::info!("✅ Deposit successfully Executed");
        //                 ()
        //             }
        //         }
        //         return;
        //     }
        // }
    }
}
