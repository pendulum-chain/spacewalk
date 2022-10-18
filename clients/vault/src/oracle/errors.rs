use std::array::TryFromSliceError;
use tokio::sync::mpsc::error::SendError;

use stellar_relay::sdk::StellarSdkError;

#[derive(Debug, err_derive::Error)]
pub enum Error {
	#[error(display = "{:?}", _0)]
	StellarSdkError(StellarSdkError),

	#[error(display = "{:?}", _0)]
	TryFromSliceError(TryFromSliceError),

	#[error(display = "{:?}", _0)]
	SerdeError(bincode::Error),

	#[error(display = "{:?}", _0)]
	StdIoError(std::io::Error),

	#[error(display = "{:?}", _0)]
	Other(String),

	#[error(display = "{:?}", _0)]
	ConnError(stellar_relay::Error),
}

impl From<StellarSdkError> for Error {
	fn from(e: StellarSdkError) -> Self {
		Error::StellarSdkError(e)
	}
}

impl From<std::io::Error> for Error {
	fn from(e: std::io::Error) -> Self {
		Error::StdIoError(e)
	}
}

impl From<bincode::Error> for Error {
	fn from(e: bincode::Error) -> Self {
		Error::SerdeError(e)
	}
}

impl From<TryFromSliceError> for Error {
	fn from(e: TryFromSliceError) -> Self {
		Error::TryFromSliceError(e)
	}
}

impl From<stellar_relay::Error> for Error {
	fn from(e: stellar_relay::Error) -> Self {
		Error::ConnError(e)
	}
}

impl<T> From<SendError<T>> for Error {
	fn from(e: SendError<T>) -> Self {
		Error::ConnError(stellar_relay::Error::SendFailed(e.to_string()))
	}
}
