use reqwest::Error as FetchError;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
	#[error("Invalid secret key")]
	InvalidSecretKey,
	#[error("Error fetching horizon data: {0}")]
	HttpFetchingError(#[from] FetchError),
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
	#[error("Could not sign envelope")]
	SignEnvelopeError,
}
