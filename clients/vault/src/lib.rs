#![recursion_limit = "256"]

mod cancellation;
mod error;
mod horizon;
mod system;
mod types;

pub mod service {
    pub use crate::cancellation::CancellationScheduler;
}
use std::time::Duration;
pub use system::{VaultService, VaultServiceConfig, ABOUT, AUTHORS, NAME, VERSION};

use runtime::InterBtcParachain;

pub use crate::{cancellation::Event, error::Error, types::IssueRequests};

/// At startup we wait until a new block has arrived before we start event listeners.
/// This constant defines the rate at which we check whether the chain height has increased.
pub const CHAIN_HEIGHT_POLLING_INTERVAL: Duration = Duration::from_millis(500);
