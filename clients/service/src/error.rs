use std::{io::Error as IoError, num::ParseIntError};

use serde_json::Error as SerdeJsonError;
use thiserror::Error;
use tokio::task::JoinError as TokioJoinError;

use runtime::Error as RuntimeError;

#[derive(Error, Debug)]
pub enum Error<InnerError> {
	#[error("Abort: {0}")]
	Abort(InnerError),
	#[error("Retry: {0}")]
	Retry(InnerError),
	#[error("Could not start service: {0}")]
	StartServiceError(InnerError),

	#[error("Client has shutdown")]
	ClientShutdown,
	#[error("OsString parsing error")]
	OsStringError,
	#[error("File already exists")]
	FileAlreadyExists,
	#[error("There is a service already running on the system, with pid {0}")]
	ServiceAlreadyRunning(u32),
	#[error("Process with pid {0} not found")]
	ProcessNotFound(String),

	#[error("SerdeJsonError: {0}")]
	SerdeJsonError(#[from] SerdeJsonError),
	#[error("ParseIntError: {0}")]
	ParseIntError(#[from] ParseIntError),
	#[error("RuntimeError: {0}")]
	RuntimeError(#[from] RuntimeError),
	#[error("TokioError: {0}")]
	TokioError(#[from] TokioJoinError),
	#[error("System I/O error: {0}")]
	IoError(#[from] IoError),
	#[error("Wallet error: {0}")]
	WalletError(#[from] wallet::error::Error),
}
