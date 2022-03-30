#![recursion_limit = "256"]

mod cancellation;
mod error;
mod execution;
mod horizon;
mod system;
mod types;

pub mod service {
    pub use crate::{
        cancellation::{CancellationScheduler, IssueCanceller, ReplaceCanceller},
        execution::execute_open_requests,
    };
}
use std::time::Duration;
pub use system::{VaultService, VaultServiceConfig, ABOUT, AUTHORS, NAME, VERSION};

use runtime::{InterBtcParachain, VaultId, VaultRegistryPallet};

pub use crate::{cancellation::Event, error::Error, types::IssueRequests};

pub(crate) async fn deposit_collateral(api: &InterBtcParachain, vault_id: &VaultId, amount: u128) -> Result<(), Error> {
    let result = api.deposit_collateral(vault_id, amount).await;
    tracing::info!("Locking additional collateral; amount {}: {:?}", amount, result);
    Ok(result?)
}

/// At startup we wait until a new block has arrived before we start event listeners.
/// This constant defines the rate at which we check whether the chain height has increased.
pub const CHAIN_HEIGHT_POLLING_INTERVAL: Duration = Duration::from_millis(500);
