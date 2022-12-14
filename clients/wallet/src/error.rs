use substrate_stellar_sdk::{horizon::FetchError, StellarSdkError};
use thiserror::Error;

#[derive(PartialEq, Clone, Debug, Error)]
pub enum Error {
	#[error("Server returned rpc error")]
	InvalidSecretKey,
	#[error("Error fetching horizon data")]
	HttpFetchingError,
	#[error("Oracle returned error")]
	OracleError,
	#[error("Could not build transaction: {0}")]
	BuildTransactionError(String),
	#[error("Transaction submission failed: {0}")]
	HorizonSubmissionError(String),
	#[error("Could not parse string: {0}")]
	Utf8Error(#[from] std::str::Utf8Error),
	#[error("Could not decode XDR")]
	DecodeError,
}
