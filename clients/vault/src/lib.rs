#![recursion_limit = "256"]

mod deposit;
mod error;
mod horizon;
mod redeem;
mod system;

pub mod oracle;

pub mod service {
	pub use crate::{
		deposit::poll_horizon_for_new_transactions, horizon::HorizonTransactionsResponse,
		redeem::listen_for_redeem_requests,
	};
}
use std::time::Duration;
pub use system::{VaultService, VaultServiceConfig, ABOUT, AUTHORS, NAME, VERSION};

pub use crate::error::Error;

/// At startup we wait until a new block has arrived before we start event listeners.
/// This constant defines the rate at which we check whether the chain height has increased.
pub const CHAIN_HEIGHT_POLLING_INTERVAL: Duration = Duration::from_millis(500);