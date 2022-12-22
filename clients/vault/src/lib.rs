#![recursion_limit = "256"]

use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;

use governor::Quota;
use nonzero_ext::*;

pub use system::{VaultIdManager, VaultService, VaultServiceConfig, ABOUT, AUTHORS, NAME, VERSION};

pub use crate::{cancellation::Event, error::Error};

mod cancellation;
mod error;
mod execution;
pub mod metrics;
pub mod process;
mod redeem;
mod replace;
mod system;

mod issue;
pub mod oracle;

pub mod service {
	pub use wallet::listen_for_new_transactions;

	pub use crate::{
		cancellation::{CancellationScheduler, IssueCanceller, ReplaceCanceller},
		execution::execute_open_requests,
		issue::{
			listen_for_executed_issues, listen_for_issue_cancels, listen_for_issue_requests,
			process_issues_requests, IssueFilter,
		},
		redeem::listen_for_redeem_requests,
		replace::{
			listen_for_accept_replace, listen_for_execute_replace, listen_for_replace_requests,
		},
	};
}

pub type ArcRwLock<T> = Arc<RwLock<T>>;

/// At startup we wait until a new block has arrived before we start event listeners.
/// This constant defines the rate at which we check whether the chain height has increased.
pub const CHAIN_HEIGHT_POLLING_INTERVAL: Duration = Duration::from_millis(500);

/// explicitly yield at most once per second
pub const YIELD_RATE: Quota = Quota::per_second(nonzero!(1u32));
