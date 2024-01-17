use rand::Rng;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use substrate_stellar_sdk::{
	types::{Error, Uint256},
	SecretKey, XdrCodec,
};
use tokio::{
	io::Interest,
	net::{tcp, TcpStream},
};

/// Returns a new BigNumber with a pseudo-random value equal to or greater than 0 and less than 1.
pub fn generate_random_nonce() -> Uint256 {
	let mut rng = rand::thread_rng();
	let random_float = rng.gen_range(0.00..1.00);
	let mut hash = Sha256::new();
	hash.update(random_float.to_string());
	hash.finalize().to_vec().try_into().unwrap()
}

pub fn secret_key_binary(key: &str) -> [u8; 32] {
	let bytes = base64::decode_config(key, base64::STANDARD).unwrap();
	let secret_key = SecretKey::from_binary(bytes.try_into().unwrap());
	secret_key.into_binary()
}

pub fn time_now() -> u64 {
	let valid_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
	u64::try_from(valid_at).unwrap_or_else(|_| {
		log::warn!("could not convert time at u128 to u64.");
		u64::MAX
	})
}

pub fn error_to_string(e: Error) -> String {
	let msg = e.msg.get_vec();
	let msg = String::from_utf8(msg.clone()).unwrap_or(format!("{:?}", e.msg.to_base64_xdr()));

	format!("Error{{ code:{:?} message:{msg} }}", e.code)
}

pub fn to_base64_xdr_string<T: XdrCodec>(msg: &T) -> String {
	let xdr = msg.to_base64_xdr();
	String::from_utf8(xdr.clone()).unwrap_or(format!("{:?}", xdr))
}

pub async fn create_stream(
	address: &str,
) -> Result<(tcp::OwnedReadHalf, tcp::OwnedWriteHalf), crate::Error> {
	let stream = TcpStream::connect(address)
		.await
		.map_err(|e| crate::Error::ConnectionFailed(e.to_string()))?;

	let res = stream.ready(Interest::READABLE | Interest::WRITABLE).await.map_err(|e| {
		log::error!("create_stream(): Stream not ready for reading or writing: {e:?}");
		crate::Error::ConnectionFailed(e.to_string())
	})?;

	if res.is_readable() {
		log::trace!("create_stream(): stream is readable");
	}

	if res.is_writable() {
		log::trace!("create_stream(): stream is writable");
	}

	if res.is_empty() {
		log::trace!("create_stream(): stream is empty");
	}

	if res.is_read_closed() {
		log::trace!("create_strea(): stream's read half is closed.");
	}

	Ok(stream.into_split())
}
