use substrate_stellar_sdk::types::MessageType;

pub const MAX_FLOOD_MSG_CAP: u32 = 200;

#[derive(Default)]
pub struct FlowController {
	enabled: bool,
	flood_msg_cap: u32,
}

impl FlowController {
	pub fn enable(&mut self, local_overlay_version: u32, remote_overlay_version: u32) {
		self.enabled = remote_overlay_version >= 20 && local_overlay_version >= 20;
	}

	pub fn send_more(&mut self, message_type: MessageType) -> bool {
		if !self.enabled {
			return false
		}

		if is_flood_message(message_type) {
			self.flood_msg_cap -= 1;
		}

		if self.flood_msg_cap == 0 {
			self.flood_msg_cap = MAX_FLOOD_MSG_CAP;
			return true
		}

		false
	}
}

pub fn is_flood_message(message_type: MessageType) -> bool {
	match message_type {
		MessageType::Transaction | MessageType::ScpMessage => true,
		_ => false,
	}
}
