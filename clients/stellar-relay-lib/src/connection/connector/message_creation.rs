use crate::connection::{
	authentication::create_auth_cert, handshake, hmac::create_sha256_hmac, xdr_converter,
	Connector, Error,
};
use substrate_stellar_sdk::{
	types::{AuthenticatedMessage, AuthenticatedMessageV0, HmacSha256Mac, StellarMessage},
	XdrCodec,
};
use substrate_stellar_sdk::compound_types::LimitedString;
use substrate_stellar_sdk::types::ErrorCode;

impl Connector {
	/// Wraps the stellar message with `AuthenticatedMessage`
	fn authenticate_message(&mut self, message: StellarMessage) -> AuthenticatedMessage {
		let mac = self.mac_for_auth_message(&message);
		let sequence = self.local_sequence();

		match &message {
			StellarMessage::ErrorMsg(_) | StellarMessage::Hello(_) => {},
			_ => {
				self.increment_local_sequence();
			},
		}

		let auth_message_v0 = AuthenticatedMessageV0 { sequence, message, mac };

		AuthenticatedMessage::V0(auth_message_v0)
	}

	pub fn create_xdr_message(&mut self, msg: StellarMessage) -> Result<Vec<u8>, Error> {
		let auth_msg = self.authenticate_message(msg);
		xdr_converter::from_authenticated_message(&auth_msg).map_err(Error::from)
	}

	/// Returns HmacSha256Mac for the AuthenticatedMessage
	fn mac_for_auth_message(&self, message: &StellarMessage) -> HmacSha256Mac {
		let empty = HmacSha256Mac { mac: [0; 32] };

		if self.remote().is_none() || self.hmac_keys().is_none() {
			return empty;
		}

		let sending_mac_key = self.hmac_keys().map(|keys| keys.sending().mac).unwrap_or([0; 32]);

		let mut buffer = self.local_sequence().to_be_bytes().to_vec();
		buffer.append(&mut message.to_xdr());
		create_sha256_hmac(&buffer, &sending_mac_key).unwrap_or(empty)
	}

	/// The hello message is dependent on the auth cert
	pub fn create_hello_message(&mut self, valid_at: u64) -> Result<StellarMessage, Error> {
		let auth_cert = match self.connection_auth.auth_cert(valid_at) {
			Ok(auth_cert) => auth_cert.clone(),
			Err(_) => {
				// depending on the error, let's create a new one.
				let new_auth_cert = create_auth_cert(
					self.connection_auth.network_id(),
					self.connection_auth.keypair(),
					valid_at,
					self.connection_auth.pub_key_ecdh().clone(),
				)?;

				self.connection_auth.set_auth_cert(new_auth_cert.clone());

				new_auth_cert
			},
		};

		let local = self.local();
		let peer_id = self.connection_auth.keypair().get_public();
		handshake::create_hello_message(
			peer_id.clone(),
			local.nonce(),
			auth_cert,
			local.port(),
			local.node(),
		)
	}
}

/// Create our own error to send over to the user/outsider.
pub(crate) fn crate_specific_error() -> StellarMessage {
	let error = "Stellar Relay Error".as_bytes().to_vec();
	let error = substrate_stellar_sdk::types::Error {
		code: ErrorCode::ErrMisc,
		msg: LimitedString::new(error).expect("should return a valid LimitedString"),
	};

	StellarMessage::ErrorMsg(error)
}