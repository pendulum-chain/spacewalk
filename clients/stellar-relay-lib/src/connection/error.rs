#![allow(dead_code)] //todo: remove after being tested and implemented

use crate::connection::xdr_converter::Error as XDRError;
use substrate_stellar_sdk::{types::ErrorCode, StellarSdkError};
use tokio::sync;
use crate::helper::error_to_string;

#[derive(Debug, err_derive::Error)]
pub enum Error {
	#[error(display = "Auth Certificate: Expired")]
	AuthCertExpired,

	#[error(display = "Auth Certificate: Not Found")]
	AuthCertNotFound,

	#[error(display = "Auth Certificate: Invalid")]
	AuthCertInvalid,

	#[error(display = "Auth Certificate Creation: Signature Failed")]
	AuthSignatureFailed,

	#[error(display = "Authentication Failed: {}", _0)]
	AuthFailed(String),

	#[error(display = "Connection: {}", _0)]
	ConnectionFailed(String),

	#[error(display = "Write: {}", _0)]
	WriteFailed(String),

	#[error(display = "Sent: {}", _0)]
	SendFailed(String),

	#[error(display = "Read: {}", _0)]
	ReadFailed(String),

	#[error(display = "The Remote Node Info wasn't initialized.")]
	NoRemoteInfo,

	#[error(display = "Sequence num with the Auth message is different with remote sequence")]
	InvalidSequenceNumber,

	#[error(display = "Hmac: {:?}", _0)]
	HmacError(hmac::digest::MacError),

	#[error(display = "Hmac: missing")]
	MissingHmacKeys,

	#[error(display = "Hmac: Invalid Length")]
	HmacInvalidLength,

	#[error(display = "{:?}", _0)]
	XDRConversionError(XDRError),

	#[error(display = "{:?}", _0)]
	StellarSdkError(StellarSdkError),

	#[error(display = "Stellar overlay disconnected")]
	Disconnected,

	#[error(display = "Config Error: {}", _0)]
	ConfigError(String),

	#[error(display = "Received Error from Overlay: {:?}", _0)]
	OverlayError(ErrorCode),

	#[error(display = "Timeout elapsed")]
	Timeout,

	#[error(display = "Config Error: Version String too long")]
	VersionStrTooLong
}

impl From<XDRError> for Error {
	fn from(e: XDRError) -> Self {
		Error::XDRConversionError(e)
	}
}

impl<T> From<sync::mpsc::error::SendError<T>> for Error {
	fn from(e: sync::mpsc::error::SendError<T>) -> Self {
		Error::SendFailed(e.to_string())
	}
}

impl From<StellarSdkError> for Error {
	fn from(e: StellarSdkError) -> Self {
		Error::StellarSdkError(e)
	}
}

impl From<substrate_stellar_sdk::types::Error> for Error {
	fn from(value: substrate_stellar_sdk::types::Error) -> Self {
		match value.code {
			ErrorCode::ErrConf => Self::ConfigError(error_to_string(value)),
			ErrorCode::ErrAuth => Self::AuthFailed(error_to_string(value)),
			other => {
				log::error!("Stellar Node returned error: {}", error_to_string(value));
				Self::OverlayError(other)
			}
		}
	}
}