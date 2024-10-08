use substrate_stellar_sdk::types::{MessageType, SendMore, SendMoreExtended, StellarMessage};

pub const PER_FLOOD_READING_CAPACITY: u32 = 200;
pub const FLOW_CONTROL_SEND_MORE_BATCH_SIZE: u32 = 40;
pub const PER_FLOOD_READING_CAPACITY_BYTES: u32 = 300000;
pub const FLOW_CONTROL_SEND_MORE_BATCH_SIZE_BYTES: u32 = 100000;

#[derive(Debug, Default)]
pub struct FlowController {
	flow_control_bytes_enabled: bool,
	messages_received_in_current_batch: u32,
	bytes_received_in_current_batch: u32,
}

impl FlowController {
	pub fn enable_bytes(&mut self, local_overlay_version: u32, remote_overlay_version: u32) {
		self.flow_control_bytes_enabled =
			remote_overlay_version >= 28 && local_overlay_version >= 28;
	}

	pub fn start(
		&mut self,
		local_overlay_version: u32,
		remote_overlay_version: u32,
	) -> StellarMessage {
		self.enable_bytes(local_overlay_version, remote_overlay_version);

		if self.flow_control_bytes_enabled {
			let msg = StellarMessage::SendMoreExtended(SendMoreExtended {
				num_messages: PER_FLOOD_READING_CAPACITY,
				num_bytes: PER_FLOOD_READING_CAPACITY_BYTES,
			});
			return msg;
		}
		let msg = StellarMessage::SendMore(SendMore { num_messages: PER_FLOOD_READING_CAPACITY });
		return msg;
	}

	fn reset_batch_counters(&mut self) {
		self.messages_received_in_current_batch = 0;
		self.bytes_received_in_current_batch = 0;
	}

	pub fn create_reclaim_capacity_message(
		&mut self,
		message_type: MessageType,
		data_len: usize,
	) -> Option<StellarMessage> {
		let data_len_u32 =
			u32::try_from(data_len).expect("data_len will always fit within u32; qed");
		let actual_message_size = data_len_u32 - 32 - 12;

		if is_flood_message(message_type) {
			self.messages_received_in_current_batch += 1;
			self.bytes_received_in_current_batch += actual_message_size;
		}

		let mut should_send_more =
			self.messages_received_in_current_batch == FLOW_CONTROL_SEND_MORE_BATCH_SIZE;

		if self.flow_control_bytes_enabled {
			should_send_more = should_send_more ||
				self.bytes_received_in_current_batch >= FLOW_CONTROL_SEND_MORE_BATCH_SIZE_BYTES;
		}

		//reclaim the capacity
		if should_send_more {
			if self.flow_control_bytes_enabled {
				let send_more_message = StellarMessage::SendMoreExtended(SendMoreExtended {
					num_messages: self.messages_received_in_current_batch, /* request back the
					                                                        * number of messages
					                                                        * we received, not
					                                                        * the total capacity
					                                                        * like when starting! */
					num_bytes: self.bytes_received_in_current_batch,
				});
				self.reset_batch_counters();

				return Some(send_more_message);
			} else {
				let send_more_message = StellarMessage::SendMore(SendMore {
					num_messages: self.messages_received_in_current_batch,
				});
				self.reset_batch_counters();

				return Some(send_more_message);
			}
		}
		return None;
	}
}

pub fn is_flood_message(message_type: MessageType) -> bool {
	match message_type {
		MessageType::Transaction |
		MessageType::ScpMessage |
		MessageType::FloodAdvert |
		MessageType::FloodDemand => true,
		_ => false,
	}
}
