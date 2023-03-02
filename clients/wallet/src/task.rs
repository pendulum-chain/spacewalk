use crate::Slot;
use tokio::sync::oneshot::{channel, error::TryRecvError, Receiver, Sender};

/// Determines the status of the task of processing a transaction
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SlotTaskStatus {
	Ready,
	/// The task ended and it was a success
	Success,
	/// The error is acceptable, and is best to just retry again
	RecoverableError,
	/// The task failed.
	Failed(String),
	/// Cannot reprocess the task since the max number of retries have been reached.
	ReachedMaxRetries,
}

/// Constructs a task dedicated to a given slot.
pub struct SlotTask {
	slot: Slot,

	/// receives the status of the task execution
	task_execution_receiver: Receiver<SlotTaskStatus>,

	/// the number of retries allowed to process this specific task
	retries_remaining: u8,

	/// The latest status of this task
	latest_status: SlotTaskStatus,
}

impl SlotTask {
	const DEFAULT_MAX_RETRIES: u8 = 3;
	/// Creates a task structure for the provided slot
	///
	/// # Arguments
	///
	/// * `slot` - the slot number for this task
	/// * `task_execution_receiver` - a oneshot receiver to receive `SlotTaskStatus` message.
	pub fn new(slot: Slot, task_execution_receiver: Receiver<SlotTaskStatus>) -> Self {
		SlotTask::new_with_max_retries(slot, task_execution_receiver, SlotTask::DEFAULT_MAX_RETRIES)
	}

	/// Creates a task structure for the provide slot, and can specify how many retries
	/// a failed (with a recoverable error) task can be executed.
	pub fn new_with_max_retries(
		slot: Slot,
		task_execution_receiver: Receiver<SlotTaskStatus>,
		max_retries: u8,
	) -> Self {
		SlotTask {
			slot,
			task_execution_receiver,
			// The decrement action is called everytime a new receiver is set.
			retries_remaining: max_retries,
			latest_status: SlotTaskStatus::Ready,
		}
	}

	pub fn slot(&self) -> Slot {
		self.slot
	}

	/// Checks the status of the task
	pub fn update_status(&mut self) -> SlotTaskStatus {
		match self.latest_status {
			// These status are considered final, and cannot be updated anymore.
			SlotTaskStatus::Success |
			SlotTaskStatus::Failed(_) |
			SlotTaskStatus::ReachedMaxRetries => {},

			// The status is immediately "ReachedMaxRetries", after exhausting the max number of
			// retries.
			_ if self.retries_remaining == 0 => {
				tracing::warn!(
					"For slot {}, maximum number of retries has been reached.",
					self.slot
				);
				self.latest_status = SlotTaskStatus::ReachedMaxRetries;
			},

			_ => {
				self.latest_status = match &self.task_execution_receiver.try_recv() {
					// Only when the retries ACTUALLY reached maximum, should the status be `ReachedMaxRetries`.
					Ok(SlotTaskStatus::ReachedMaxRetries) |
					// The status can only be "Ready" during initialization AND when calling the method
					// `recover_with_new_sender`
					Ok(SlotTaskStatus::Ready)=> {
						tracing::warn!("For slot {}, user can only send the ff. status: Success, Failed, RecoverableError", self.slot);
						self.latest_status.clone()
					}
					Ok(new_status) => { new_status.clone() },
					Err(TryRecvError::Empty) => SlotTaskStatus::Ready,
					Err(TryRecvError::Closed) => SlotTaskStatus::RecoverableError
				};
			},
		}

		self.latest_status.clone()
	}

	/// Sets a new one shot receiver.
	/// Returns true if it was successful.
	fn set_receiver(&mut self, receiver: Receiver<SlotTaskStatus>) -> bool {
		match self.latest_status {
			SlotTaskStatus::Ready | SlotTaskStatus::RecoverableError
				if self.retries_remaining > 0 =>
			{
				self.task_execution_receiver = receiver;
				self.retries_remaining = self.retries_remaining.saturating_sub(1);
				true
			},
			// Change of status is NOT allowed for tasks with latest status of either "Success" or
			// "Failed". "Success" and "Failed" status are considered final.
			_ => false,
		}
	}

	/// Returns oneshot sender for sending status of a ledger
	/// if and only if the status is `RecoverableError`
	pub fn recover_with_new_sender(&mut self) -> Option<Sender<SlotTaskStatus>> {
		// Only recoverable errors can be given a new task.
		if self.update_status() == SlotTaskStatus::RecoverableError {
			let (sender, receiver) = channel();

			if self.set_receiver(receiver) {
				self.latest_status = SlotTaskStatus::Ready;
				return Some(sender)
			}
		}

		// For tasks flagged as `Ready` , wait for it.
		// For tasks flagged as `Success`, they will be removed in the for loop
		// For tasks flagged as `Error`, these will be removed.
		None
	}
}

#[cfg(test)]
mod test {
	use super::*;

	fn dummy_error() -> String {
		"this is a test".to_string()
	}

	#[test]
	fn test_task_creation() {
		{
			let slot = 10;
			let (_, receiver) = channel();

			let task = SlotTask::new(slot, receiver);

			assert_eq!(task.slot, slot);
			assert_eq!(task.retries_remaining, SlotTask::DEFAULT_MAX_RETRIES);
			assert_eq!(task.latest_status, SlotTaskStatus::Ready);
		}
		{
			let slot = 50;
			let max_retries = 10;
			let (_, receiver) = channel();
			let task = SlotTask::new_with_max_retries(slot, receiver, max_retries);
			assert_eq!(task.slot, slot);
			assert_eq!(task.retries_remaining, max_retries);
			assert_eq!(task.latest_status, SlotTaskStatus::Ready);
		}
	}

	#[test]
	fn status_change_success() {
		{
			let (sender, receiver) = channel();
			let mut task = SlotTask::new(15, receiver);

			sender
				.send(SlotTaskStatus::Success)
				.expect("should be able to send Success status");
			assert_eq!(task.update_status(), SlotTaskStatus::Success);
		}
		{
			let (sender, receiver) = channel();
			let mut task = SlotTask::new(20, receiver);

			sender
				.send(SlotTaskStatus::Failed(dummy_error()))
				.expect("should be able to send Failed status");
			assert_eq!(task.update_status(), SlotTaskStatus::Failed(dummy_error()));
		}
	}

	#[test]
	fn status_change_failed() {
		{
			let (sender, receiver) = channel();
			let mut task = SlotTask::new(40, receiver);

			sender
				.send(SlotTaskStatus::ReachedMaxRetries)
				.expect("should be able to send status");
			assert_ne!(task.update_status(), SlotTaskStatus::ReachedMaxRetries);
		}
		{
			let (sender, receiver) = channel();
			let mut task = SlotTask::new(40, receiver);

			sender.send(SlotTaskStatus::Ready).expect("should be able to send status");
			// actually the status remains the same. It's just that the "change" was never made.
			assert_eq!(task.update_status(), SlotTaskStatus::Ready);
		}
	}

	#[test]
	fn recover_with_new_sender_success() {
		{
			let (sender, receiver) = channel();
			let mut task = SlotTask::new(10, receiver);

			sender
				.send(SlotTaskStatus::RecoverableError)
				.expect("should be able to send a RecoverableError status");
			assert_eq!(task.update_status(), SlotTaskStatus::RecoverableError);

			// let's try to recover:
			let new_sender = task.recover_with_new_sender().expect("should return a sender");
			assert_eq!(task.update_status(), SlotTaskStatus::Ready);

			new_sender
				.send(SlotTaskStatus::Success)
				.expect("should be able to send a status");
			assert_eq!(task.update_status(), SlotTaskStatus::Success);
		}

		{
			let (sender, receiver) = channel();
			let mut task = SlotTask::new(12, receiver);

			sender
				.send(SlotTaskStatus::RecoverableError)
				.expect("should be able to send a status");
			assert_eq!(task.update_status(), SlotTaskStatus::RecoverableError);

			// let's try to recover:
			let new_sender = task.recover_with_new_sender().expect("should return a sender");
			assert_eq!(task.update_status(), SlotTaskStatus::Ready);

			new_sender
				.send(SlotTaskStatus::Failed(dummy_error()))
				.expect("should be able to send a status");
			assert_ne!(task.update_status(), SlotTaskStatus::Ready);
		}
	}

	#[test]
	fn recover_with_new_sender_failed() {
		{
			let (sender, receiver) = channel();
			let mut task = SlotTask::new(5, receiver);

			sender.send(SlotTaskStatus::Success).expect("should be able to send");
			assert!(task.recover_with_new_sender().is_none());
		}
		{
			let (sender, receiver) = channel();
			let mut task = SlotTask::new(5, receiver);

			sender
				.send(SlotTaskStatus::Failed(dummy_error()))
				.expect("should be able to send");
			assert!(task.recover_with_new_sender().is_none());
		}
	}

	#[test]
	fn max_retries_exhausted() {
		fn exhaustion_test(sender: Sender<SlotTaskStatus>, mut task: SlotTask, max_retries: u8) {
			let mut sender = sender;
			// let's exhaust the retries
			for _ in 0..max_retries {
				assert_eq!(task.update_status(), SlotTaskStatus::Ready);
				sender.send(SlotTaskStatus::RecoverableError).expect("should send status");
				sender = task.recover_with_new_sender().expect("should return a sender");
			}
			assert_eq!(task.retries_remaining, 0);
			assert_eq!(task.update_status(), SlotTaskStatus::ReachedMaxRetries);
		}

		{
			let (sender, receiver) = channel();
			let task = SlotTask::new(123, receiver);

			exhaustion_test(sender, task, SlotTask::DEFAULT_MAX_RETRIES);
		}
		{
			let max_retries = 8;
			let (sender, receiver) = channel();
			let task = SlotTask::new_with_max_retries(321, receiver, max_retries);

			exhaustion_test(sender, task, max_retries)
		}
	}
}
