use thiserror::Error;

#[derive(PartialEq, Eq, Clone, Debug, Error)]
pub enum Error {
	#[error("Server returned rpc error")]
	InvalidSecretKey,
	#[error("Error fetching horizon data")]
	HttpFetchingError,
	#[error("Oracle returned error")]
	OracleError,
}
