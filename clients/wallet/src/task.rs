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
}

/// Constructs a task dedicated to a given slot.
pub struct SlotTask {
	slot: Slot,
	/// receives the status of the task execution
	task_execution_receiver: Receiver<SlotTaskStatus>,
	/// The latest status of this task
	latest_status: SlotTaskStatus,
}

impl SlotTask {
	/// Creates a task structure for the provided slot
	///
	/// # Arguments
	///
	/// * `slot` - the slot number for this task
	/// * `task_execution_receiver` - a oneshot receiver to receive `SlotTaskStatus` message.
	pub fn new(slot: Slot, task_execution_receiver: Receiver<SlotTaskStatus>) -> Self {
		SlotTask { slot, task_execution_receiver, latest_status: SlotTaskStatus::Ready }
	}

	pub fn slot(&self) -> Slot {
		self.slot
	}

	/// Checks the status of the task
	pub fn update_status(&mut self) -> SlotTaskStatus {
		match self.latest_status {
			// These status are considered final, and cannot be updated anymore.
			SlotTaskStatus::Success | SlotTaskStatus::Failed(_) => {},

			_ => {
				self.latest_status = match &self.task_execution_receiver.try_recv() {
					// The status can only be "Ready" during initialization AND when calling the
					// method `recover_with_new_sender`
					Ok(SlotTaskStatus::Ready) => {
						tracing::warn!("For slot {}, user can only send the ff. status: Success, Failed, RecoverableError", self.slot);
						self.latest_status.clone()
					},
					Ok(new_status) => new_status.clone(),
					Err(TryRecvError::Empty) => SlotTaskStatus::Ready,
					Err(TryRecvError::Closed) => SlotTaskStatus::RecoverableError,
				};
			},
		}

		self.latest_status.clone()
	}

	/// Sets a new one shot receiver.
	/// Returns true if it was successful.
	fn set_receiver(&mut self, receiver: Receiver<SlotTaskStatus>) -> bool {
		match self.latest_status {
			SlotTaskStatus::Ready | SlotTaskStatus::RecoverableError => {
				self.task_execution_receiver = receiver;
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
				tracing::debug!("Creating new sender for failed task of slot {}", self.slot);

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
			assert_eq!(task.latest_status, SlotTaskStatus::Ready);
		}
		{
			let slot = 50;
			let (_, receiver) = channel();
			let task = SlotTask::new(slot, receiver);
			assert_eq!(task.slot, slot);
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
			task.latest_status = SlotTaskStatus::Success;

			sender
				.send(SlotTaskStatus::RecoverableError)
				.expect("should be able to send status");
			assert_ne!(task.update_status(), SlotTaskStatus::RecoverableError);
		}
		{
			let (sender, receiver) = channel();
			let mut task = SlotTask::new(40, receiver);
			task.latest_status = SlotTaskStatus::Failed(dummy_error());

			sender.send(SlotTaskStatus::Success).expect("should be able to send status");
			assert_ne!(task.update_status(), SlotTaskStatus::Success);
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
}
