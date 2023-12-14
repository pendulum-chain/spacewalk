use crate::{
	connection::authentication::{BinarySha256Hash, ConnectionAuth, AUTH_CERT_EXPIRATION_LIMIT},
	Error,
};
use sha2::{Digest, Sha256};
use substrate_stellar_sdk::{
	types::{AuthCert, Curve25519Public, EnvelopeType, Signature},
	PublicKey, SecretKey, XdrCodec,
};

impl ConnectionAuth {
	///  Returns none if the validity start date exceeds the expiration
	///  or if an auth cert was never created in the first place.
	///
	/// # Arguments
	/// * `valid_at` - the validity start date in milliseconds.
	pub fn auth_cert(&self, valid_at: u64) -> Result<&AuthCert, Error> {
		self.saved_auth_cert().ok_or(Error::AuthCertNotFound).and_then(|auth_cert| {
			if self.auth_cert_expiration() < (valid_at + AUTH_CERT_EXPIRATION_LIMIT / 2) {
				Err(Error::AuthCertExpired)
			} else {
				Ok(auth_cert)
			}
		})
	}

	pub fn set_auth_cert(&mut self, auth_cert: AuthCert) {
		self.update_auth_cert_expiration(auth_cert.expiration);
		self.update_auth_cert(auth_cert);
	}
}

pub fn create_auth_cert(
	network_id: &BinarySha256Hash,
	keypair: &SecretKey,
	valid_at: u64,
	pub_key_ecdh: Curve25519Public,
) -> Result<AuthCert, Error> {
	let mut network_id_xdr = network_id.to_xdr();
	let expiration = valid_at + AUTH_CERT_EXPIRATION_LIMIT;

	let mut buf: Vec<u8> = vec![];

	buf.append(&mut network_id_xdr);
	buf.append(&mut EnvelopeType::EnvelopeTypeAuth.to_xdr());
	buf.append(&mut expiration.to_xdr());
	buf.append(&mut pub_key_ecdh.key.to_vec());

	let mut hash = Sha256::new();
	hash.update(buf);

	let raw_sig_data = hash.finalize().to_vec();

	let signature: Signature = Signature::new(keypair.create_signature(raw_sig_data).to_vec())?;

	Ok(AuthCert { pubkey: pub_key_ecdh, expiration, sig: signature })
}

pub fn verify_remote_auth_cert(
	time_in_millisecs: u64,
	remote_pub_key: &PublicKey,
	auth_cert: &AuthCert,
	network_id_xdr: &mut [u8],
) -> bool {
	let expiration = auth_cert.expiration;
	if expiration <= (time_in_millisecs / 1000) {
		return false
	}

	let mut raw_data: Vec<u8> = vec![];
	raw_data.extend_from_slice(network_id_xdr);
	raw_data.append(&mut EnvelopeType::EnvelopeTypeAuth.to_xdr());
	raw_data.append(&mut auth_cert.expiration.to_xdr());
	raw_data.append(&mut auth_cert.pubkey.key.to_vec());

	let mut hash = Sha256::new();
	hash.update(raw_data);

	let raw_data = hash.finalize().to_vec();
	let auth_cert_sig = auth_cert.sig.get_vec().clone();
	let sig_len = auth_cert_sig.len();

	match auth_cert_sig.try_into() {
		Ok(raw_sig) => remote_pub_key.verify_signature(raw_data, &raw_sig),
		Err(_) => {
			log::warn!(
				"failed to convert auth cert signature of size {} to fixed array of 64.",
				sig_len
			);
			false
		},
	}
}
