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
	#[error("Could not build transaction")]
	BuildTransactionError,
	#[error("Transaction submission failed")]
	HorizonSubmissionError,
}
