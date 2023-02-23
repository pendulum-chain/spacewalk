use std::collections::HashMap;
use substrate_stellar_sdk::{TransactionEnvelope, XdrCodec};
use tokio::sync::oneshot::error::TryRecvError;

pub use horizon::{listen_for_new_transactions, TransactionResponse};
pub use stellar_wallet::StellarWallet;

pub mod error;
mod horizon;
mod stellar_wallet;
pub mod types;

pub type Ledger = u32;

/// Determines the status of the task of processing a transaction
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TxEnvTaskStatus {
	NotStarted,
	/// Process Done and it was a success
	ProcessSuccess,
	/// The error is acceptable, and is best to just retry again
	RecoverableError,
	/// Something happened when
	Error(String),
}

pub struct LedgerTask {
	/// the
	ledger: Ledger,

	/// receives the status of the task execution
	task_execution_receiver: tokio::sync::oneshot::Receiver<TxEnvTaskStatus>,

	/// the number of retries allowed to process this specific task
	retries_remaining: u8,

	/// The latest status of this task
	latest_status: TxEnvTaskStatus,
}

impl LedgerTask {
	/// Creates a task structure the `TransactionEnvelope`
	pub fn create(
		ledger: Ledger,
		task_execution_receiver: tokio::sync::oneshot::Receiver<TxEnvTaskStatus>,
	) -> Self {
		LedgerTask {
			ledger,
			task_execution_receiver,
			// The decrement action is called everytime a new receiver is set.
			retries_remaining: 3,
			latest_status: TxEnvTaskStatus::NotStarted,
		}
	}

	pub fn set_status(&mut self, status: TxEnvTaskStatus) {
		self.latest_status = status;
	}

	/// Set the one shot receiver.
	/// Returns true if it was successful.
	pub fn set_receiver(
		&mut self,
		receiver: tokio::sync::oneshot::Receiver<TxEnvTaskStatus>,
	) -> bool {
		match self.latest_status {
			TxEnvTaskStatus::NotStarted | TxEnvTaskStatus::RecoverableError
				if self.retries_remaining > 0 =>
			{
				self.task_execution_receiver = receiver;
				self.retries_remaining = self.retries_remaining.saturating_sub(1);
				true
			},
			_ => false,
		}
	}

	pub fn ledger(&self) -> Ledger {
		self.ledger
	}

	/// Checks the status of the task
	pub fn check_status(&mut self) -> TxEnvTaskStatus {
		if self.retries_remaining == 0 {
			TxEnvTaskStatus::Error(format!(
				"Transaction for ledger {} reached max limit of retries.",
				self.ledger
			));
		}

		match &self.task_execution_receiver.try_recv() {
			Ok(res) => res.clone(),
			Err(TryRecvError::Empty) => TxEnvTaskStatus::NotStarted,
			Err(TryRecvError::Closed) => TxEnvTaskStatus::RecoverableError,
		};

		self.latest_status.clone()
	}
}

pub type LedgerTxEnvMap = HashMap<u32, TransactionEnvelope>;
