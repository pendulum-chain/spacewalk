use crate::Ledger;
use tokio::sync::oneshot::{channel, error::TryRecvError, Receiver, Sender};

pub const LEDGER_TASK_MAX_RETRIES: u8 = 3;
/// Determines the status of the task of processing a transaction
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LedgerTaskStatus {
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
	task_execution_receiver: Receiver<LedgerTaskStatus>,

	/// the number of retries allowed to process this specific task
	retries_remaining: u8,

	/// The latest status of this task
	latest_status: LedgerTaskStatus,
}

impl LedgerTask {
	/// Creates a task structure the `TransactionEnvelope`
	pub fn create(ledger: Ledger, task_execution_receiver: Receiver<LedgerTaskStatus>) -> Self {
		LedgerTask {
			ledger,
			task_execution_receiver,
			// The decrement action is called everytime a new receiver is set.
			retries_remaining: LEDGER_TASK_MAX_RETRIES,
			latest_status: LedgerTaskStatus::NotStarted,
		}
	}

	/// Sets a new one shot receiver.
	/// Returns true if it was successful.
	pub fn change_receiver(&mut self, receiver: Receiver<LedgerTaskStatus>) -> bool {
		match self.latest_status {
			LedgerTaskStatus::NotStarted | LedgerTaskStatus::RecoverableError
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
	pub fn status(&mut self) -> LedgerTaskStatus {
		if self.retries_remaining == 0 {
			self.latest_status = LedgerTaskStatus::Error(format!(
				"Transaction for ledger {} reached max limit of retries.",
				self.ledger
			));
		} else {
			self.latest_status = match &self.task_execution_receiver.try_recv() {
				Ok(new_status) => match self.latest_status {
					LedgerTaskStatus::NotStarted | LedgerTaskStatus::RecoverableError =>
						new_status.clone(),
					LedgerTaskStatus::ProcessSuccess | LedgerTaskStatus::Error(_) =>
						self.latest_status.clone(),
				},
				Err(TryRecvError::Empty) => LedgerTaskStatus::NotStarted,
				Err(TryRecvError::Closed) => LedgerTaskStatus::RecoverableError,
			};
		}

		self.latest_status.clone()
	}

	/// Returns oneshot sender for sending status of a ledger
	/// if and only if the status is `RecoverableError`
	pub fn recover_and_create_sender(&mut self) -> Option<Sender<LedgerTaskStatus>> {
		// Only recoverable errors can be given a new task.
		if self.status() == LedgerTaskStatus::RecoverableError {
			let (sender, receiver) = channel();

			if self.change_receiver(receiver) {
				self.latest_status = LedgerTaskStatus::NotStarted;
				return Some(sender)
			}
		}

		// For tasks flagged as `NotStarted` , wait for it.
		// For tasks flagged as `ProcessSuccess`, they will be removed in the for loop
		// For tasks flagged as `Error`, these will be removed.
		// continue with the loop
		None
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_create() {
		let ledger = 10;
		let (sender, receiver) = channel();

		let task = LedgerTask::create(ledger, receiver);

		assert_eq!(task.ledger, ledger);
		assert_eq!(task.retries_remaining, LEDGER_TASK_MAX_RETRIES);
		assert_eq!(task.latest_status, LedgerTaskStatus::NotStarted);

		drop(sender);
	}

	#[test]
	fn test_status() {
		{
			let (sender, receiver) = channel();
			let mut task = LedgerTask::create(15, receiver);

			sender
				.send(LedgerTaskStatus::ProcessSuccess)
				.expect("should be able to send status");
			assert_eq!(task.status(), LedgerTaskStatus::ProcessSuccess);
		}
		{
			let (sender, receiver) = channel();
			let mut task = LedgerTask::create(20, receiver);

			sender
				.send(LedgerTaskStatus::Error("this is a test".to_string()))
				.expect("should be able to send status");
			assert_eq!(task.status(), LedgerTaskStatus::Error("this is a test".to_string()));
		}
	}

	#[test]
	fn recover_and_create_sender_success() {
		{
			let ledger = 10;
			let (sender, receiver) = channel();
			let mut task = LedgerTask::create(ledger, receiver);

			sender
				.send(LedgerTaskStatus::RecoverableError)
				.expect("should be able to send a status");
			assert_eq!(task.status(), LedgerTaskStatus::RecoverableError);

			// let's try to recover:
			let new_sender = task.recover_and_create_sender().expect("should return a sender");
			assert_eq!(task.status(), LedgerTaskStatus::NotStarted);

			new_sender
				.send(LedgerTaskStatus::ProcessSuccess)
				.expect("should be able to send a status");
			assert_eq!(task.status(), LedgerTaskStatus::ProcessSuccess);
		}

		{
			let (sender, receiver) = channel();
			let mut task = LedgerTask::create(12, receiver);

			sender
				.send(LedgerTaskStatus::RecoverableError)
				.expect("should be able to send a status");
			assert_eq!(task.status(), LedgerTaskStatus::RecoverableError);

			// let's try to recover:
			let new_sender = task.recover_and_create_sender().expect("should return a sender");
			assert_eq!(task.status(), LedgerTaskStatus::NotStarted);

			new_sender
				.send(LedgerTaskStatus::Error("this is a test".to_string()))
				.expect("should be able to send a status");
			assert_ne!(task.status(), LedgerTaskStatus::NotStarted);
		}
	}

	#[test]
	fn recover_and_create_sender_failed() {
		{
			let (sender, receiver) = channel();
			let mut task = LedgerTask::create(5, receiver);

			sender.send(LedgerTaskStatus::ProcessSuccess).expect("should be able to send");
			assert!(task.recover_and_create_sender().is_none());
		}
		{
			let (sender, receiver) = channel();
			let mut task = LedgerTask::create(5, receiver);

			sender
				.send(LedgerTaskStatus::Error("this is a test".to_string()))
				.expect("should be able to send");
			assert!(task.recover_and_create_sender().is_none());
		}
	}
}
