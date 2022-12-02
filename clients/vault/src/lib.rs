#![recursion_limit = "256"]

use std::time::Duration;

pub use system::{VaultIdManager, VaultService, VaultServiceConfig, ABOUT, AUTHORS, NAME, VERSION};

pub use crate::error::Error;

mod error;
mod execution;
pub mod metrics;
pub mod process;
mod system;

// Used for integration test
pub use system::inner_create_handler;

mod issue;
pub mod oracle;

pub mod service {}

/// At startup we wait until a new block has arrived before we start event listeners.
/// This constant defines the rate at which we check whether the chain height has increased.
pub const CHAIN_HEIGHT_POLLING_INTERVAL: Duration = Duration::from_millis(500);
