use std::array::TryFromSliceError;

use tokio::sync::{mpsc, oneshot};

use stellar_relay_lib::sdk::StellarSdkError;

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
	ConnError(stellar_relay_lib::Error),

	#[error(display = "{:?}", _0)]
	WalletError(wallet::error::Error),

	#[error(display = "{:?}", _0)]
	ProofTimeout(String),
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

impl From<stellar_relay_lib::Error> for Error {
	fn from(e: stellar_relay_lib::Error) -> Self {
		Error::ConnError(e)
	}
}

impl<T> From<mpsc::error::SendError<T>> for Error {
	fn from(e: mpsc::error::SendError<T>) -> Self {
		Error::ConnError(stellar_relay_lib::Error::SendFailed(e.to_string()))
	}
}

impl From<oneshot::error::RecvError> for Error {
	fn from(e: oneshot::error::RecvError) -> Self {
		Error::ConnError(stellar_relay_lib::Error::SendFailed(e.to_string()))
	}
}
