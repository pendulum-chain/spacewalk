use std::collections::HashMap;
use substrate_stellar_sdk::TransactionEnvelope;

pub use horizon::{listen_for_new_transactions, TransactionResponse, Balance};
pub use stellar_wallet::StellarWallet;
pub use task::*;

pub mod error;
mod horizon;
mod stellar_wallet;
mod task;
pub mod types;

pub type Slot = u32;
pub type LedgerTxEnvMap = HashMap<Slot, TransactionEnvelope>;
