use std::collections::HashMap;
use substrate_stellar_sdk::TransactionEnvelope;

pub use horizon::{listen_for_new_transactions, Balance, TransactionResponse};
pub use stellar_wallet::StellarWallet;
pub use task::*;

mod cache;
pub mod error;
mod horizon;
mod operations;
mod stellar_wallet;
mod task;
pub mod types;

pub type Slot = u32;
pub type LedgerTxEnvMap = HashMap<Slot, TransactionEnvelope>;
