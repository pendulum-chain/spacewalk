use std::array::TryFromSliceError;

use tokio::sync::{mpsc, oneshot};

use stellar_relay_lib::sdk::StellarSdkError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Stellar SDK Error: {0:?}")]
	StellarSdkError(StellarSdkError),

	#[error("TryFromSliceError: {0}")]
	TryFromSliceError(#[from] TryFromSliceError),

	#[error("Serde Error: {0}")]
	SerdeError(#[from] bincode::Error),

	#[error("StdIoError: {0}")]
	StdIoError(#[from] std::io::Error),

	#[error("Other: {0}")]
	Other(String),

	#[error("Stellar Relay Error: {0}")]
	ConnError(#[from] stellar_relay_lib::Error),

	#[error("Wallet Error: {0}")]
	WalletError(#[from] wallet::error::Error),

	#[error("Proof Timeout: {0}")]
	ProofTimeout(String),

	#[error("Unititialized: {0}")]
	Uninitialized(String),

	#[error("Archive Error: {0}")]
	ArchiveError(String),

	#[error("ArchiveResponseError: {0}")]
	ArchiveResponseError(String),
}

impl From<StellarSdkError> for Error {
	fn from(e: StellarSdkError) -> Self {
		Error::StellarSdkError(e)
	}
}
//
// impl From<std::io::Error> for Error {
// 	fn from(e: std::io::Error) -> Self {
// 		Error::StdIoError(e)
// 	}
// }
//
// impl From<bincode::Error> for Error {
// 	fn from(e: bincode::Error) -> Self {
// 		Error::SerdeError(e)
// 	}
// }
//
// impl From<TryFromSliceError> for Error {
// 	fn from(e: TryFromSliceError) -> Self {
// 		Error::TryFromSliceError(e)
// 	}
// }
//
// impl From<stellar_relay_lib::Error> for Error {
// 	fn from(e: stellar_relay_lib::Error) -> Self {
// 		Error::ConnError(e)
// 	}
// }

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