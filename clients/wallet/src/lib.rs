use std::collections::HashMap;
use substrate_stellar_sdk::TransactionEnvelope;

pub use horizon::{listen_for_new_transactions, TransactionResponse};
pub use stellar_wallet::StellarWallet;

pub mod error;
mod horizon;
mod stellar_wallet;
pub mod types;

pub type Ledger = u32;
pub type LedgerTxEnvMap = HashMap<u32, TransactionEnvelope>;
