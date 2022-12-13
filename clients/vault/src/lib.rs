#![recursion_limit = "256"]

use std::time::Duration;

pub use system::{VaultIdManager, VaultService, VaultServiceConfig, ABOUT, AUTHORS, NAME, VERSION};
// Used for integration test
pub use system::inner_create_handler;

pub use crate::{cancellation::Event, error::Error};

mod cancellation;
mod error;
mod execution;
pub mod metrics;
pub mod process;
mod redeem;
mod system;

mod issue;
pub mod oracle;

pub mod service {
	pub use wallet::listen_for_new_transactions;

	pub use crate::{
		cancellation::{CancellationScheduler, IssueCanceller, ReplaceCanceller},
		issue::{
			listen_for_executed_issues, listen_for_issue_cancels, listen_for_issue_requests,
			process_issues_with_proofs, IssueFilter,
		},
		redeem::listen_for_redeem_requests,
	};
}

/// At startup we wait until a new block has arrived before we start event listeners.
/// This constant defines the rate at which we check whether the chain height has increased.
pub const CHAIN_HEIGHT_POLLING_INTERVAL: Duration = Duration::from_millis(500);
