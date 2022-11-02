use crate::Error;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use substrate_stellar_sdk::types::{HmacSha256Mac, Uint256};

pub struct HMacKeys {
	sending: HmacSha256Mac,
	receiving: HmacSha256Mac,
}

impl HMacKeys {
	pub fn new(
		shared_key: &HmacSha256Mac,
		local_nonce: Uint256,
		remote_nonce: Uint256,
		remote_called_us: bool,
	) -> Self {
		let sending =
			create_sending_mac_key(&shared_key, local_nonce, remote_nonce, !remote_called_us);

		let receiving =
			create_receiving_mac_key(&shared_key, local_nonce, remote_nonce, !remote_called_us);

		HMacKeys { sending, receiving }
	}

	pub fn sending(&self) -> &HmacSha256Mac {
		&self.sending
	}

	pub fn receiving(&self) -> &HmacSha256Mac {
		&self.receiving
	}
}

pub fn create_sending_mac_key(
	shared_key: &HmacSha256Mac,
	local_nonce: Uint256,
	remote_nonce: Uint256,
	we_called_remote: bool,
) -> HmacSha256Mac {
	let mut buf: Vec<u8> = vec![];

	if we_called_remote {
		buf.append(&mut vec![0]);
	} else {
		buf.append(&mut vec![1]);
	}

	let mut local_n = local_nonce.to_vec();
	let mut remote_n = remote_nonce.to_vec();

	buf.append(&mut local_n);
	buf.append(&mut remote_n);
	buf.append(&mut vec![1]);

	create_sha256_hmac(&buf, &shared_key.mac).unwrap_or(HmacSha256Mac { mac: [0; 32] })
}

pub fn create_receiving_mac_key(
	shared_key: &HmacSha256Mac,
	local_nonce: Uint256,
	remote_nonce: Uint256,
	we_called_remote: bool,
) -> HmacSha256Mac {
	let mut buf: Vec<u8> = vec![];

	if we_called_remote {
		buf.append(&mut vec![1]);
	} else {
		buf.append(&mut vec![0]);
	}

	let mut local_n = local_nonce.to_vec();
	let mut remote_n = remote_nonce.to_vec();

	buf.append(&mut remote_n);
	buf.append(&mut local_n);
	buf.append(&mut vec![1]);

	create_sha256_hmac(&buf, &shared_key.mac).unwrap_or(HmacSha256Mac { mac: [0; 32] })
}

type Buffer = [u8; 32];

// Create alias for HMAC-SHA256
pub type HmacSha256 = Hmac<Sha256>;

pub fn create_sha256_hmac(data_buffer: &[u8], mac_key_buffer: &Buffer) -> Option<HmacSha256Mac> {
	if let Ok(mut hmac) = HmacSha256::new_from_slice(mac_key_buffer) {
		hmac.update(data_buffer);

		let hmac_vec = hmac.finalize().into_bytes().to_vec();
		let hmac_vec_len = hmac_vec.len();
		return match hmac_vec.try_into() {
			Ok(mac) => Some(HmacSha256Mac { mac }),
			Err(_) => {
				log::warn!("failed to convert hmac of size {} into an array of 32.", hmac_vec_len);
				None
			},
		}
	}

	log::warn!("Invalid length of mac key buffer size {}", mac_key_buffer.len());
	None
}

pub fn verify_hmac(data_buffer: &[u8], mac_key_buffer: &Buffer, mac: &[u8]) -> Result<(), Error> {
	let mut hmac =
		HmacSha256::new_from_slice(mac_key_buffer).map_err(|_| Error::HmacInvalidLength)?;

	hmac.update(data_buffer);
	hmac.verify_slice(mac).map_err(|e| Error::HmacError(e))
}
