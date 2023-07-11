use std::collections::HashMap;
use substrate_stellar_sdk::{Asset, Operation, PublicKey, TransactionEnvelope};
use primitives::StellarStroops;
use crate::operations::redeem::RedeemOps;
pub use horizon::{listen_for_new_transactions, Balance, TransactionResponse};

pub use stellar_wallet::StellarWallet;
pub use task::*;

mod cache;
pub mod error;
mod horizon;
mod stellar_wallet;
mod task;
pub mod types;
mod operations;

pub type Slot = u32;
pub type LedgerTxEnvMap = HashMap<Slot, TransactionEnvelope>;


pub async fn is_claimable_balance_op_required(
    destination_address:PublicKey,
    is_public_network:bool,
    to_be_redeemed_asset:Asset,
    to_be_redeemed_amount: StellarStroops
) -> Result<Operation,Vec<Operation>> {
    let horizon_client = reqwest::Client::new();

    horizon_client.claimable_balance_ops_required(
        destination_address,
        is_public_network,
        to_be_redeemed_asset,
        to_be_redeemed_amount
    ).await
}