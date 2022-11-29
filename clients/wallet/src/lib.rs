pub use stellar_wallet::{StellarWallet, Watcher};

pub mod error;
mod horizon;
mod stellar_wallet;

pub use horizon::listen_for_new_transactions;
