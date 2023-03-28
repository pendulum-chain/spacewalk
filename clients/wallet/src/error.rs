use reqwest::Error as FetchError;
use substrate_stellar_sdk::{types::SequenceNumber, TransactionEnvelope};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
	#[error("Invalid secret key")]
	InvalidSecretKey,
	#[error("Error fetching horizon data: {0}")]
	HttpFetchingError(#[from] FetchError),
	#[error("Could not build transaction: {0}")]
	BuildTransactionError(String),
	#[error("Transaction submission failed. Title: {title}, Status: {status}, Reason: {reason}, Envelope XDR: {envelope_xdr}")]
	HorizonSubmissionError { title: String, status: u16, reason: String, envelope_xdr: String },
	#[error("Could not parse string: {0}")]
	Utf8Error(#[from] std::str::Utf8Error),
	#[error("Could not decode XDR")]
	DecodeError,
	#[error("Could not sign envelope")]
	SignEnvelopeError,

	// Cache Errors
	#[error("Cache Error: Could not extract sequence number from transaction: {0:?}.")]
	UnknownSequenceNumber(TransactionEnvelope),

	#[error("Cache Error: Sequence number {0} already in use.")]
	SequenceNumberAlreadyUsed(SequenceNumber),

	#[error("Cache Error: Failed to create directory for this path:{0}.")]
	CreateDirectoryFailed(String),

	#[error("Cache Error: Failed to create a file for {envelope:?} with path:{path}.")]
	FileCreationFailed { path: String, envelope: TransactionEnvelope },

	#[error("Cache Error: Failed to write a file for {envelope:?} with path:{path}.")]
	WriteToFileFailed { path: String, envelope: TransactionEnvelope },

	#[error("Cache Error: Failed to remove file: {0:?}")]
	DeleteFileFailed(String),

	#[error("Cache Error: File: {0:?} does not exist")]
	FileDoesNotExist(String),

	#[error("Cache Error: Cannot read file: {0:?}")]
	ReadFileFailed(String),

	#[error("Cache Error: Invalid file: {0:?}")]
	InvalidFile(String),

	#[error("Cache Error: Cannot decode xdr format for file: {0:?}")]
	DecodeFileFailed(String),
}
