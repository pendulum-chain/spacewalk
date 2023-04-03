use reqwest::Error as FetchError;
use std::fmt::{Debug, Display, Formatter};
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
	#[error("Transaction submission failed. Title: {title}, Status: {status}, Reason: {reason}, Envelope XDR: {envelope_xdr:?}")]
	HorizonSubmissionError {
		title: String,
		status: u16,
		reason: String,
		envelope_xdr: Option<String>,
	},
	#[error("Could not parse string: {0}")]
	Utf8Error(#[from] std::str::Utf8Error),
	#[error("Could not decode XDR")]
	DecodeError,
	#[error("Could not sign envelope")]
	SignEnvelopeError,

	#[error(transparent)]
	CacheError(CacheError),
}

impl Error {
	pub fn cache_error(kind: CacheErrorKind) -> Self {
		Self::CacheError(CacheError { kind, path: None, envelope: None, sequence_number: None })
	}

	pub fn cache_error_with_path(kind: CacheErrorKind, path: String) -> Self {
		Self::CacheError(CacheError {
			kind,
			path: Some(path),
			envelope: None,
			sequence_number: None,
		})
	}

	pub fn cache_error_with_seq(kind: CacheErrorKind, sequence_number: SequenceNumber) -> Self {
		Self::CacheError(CacheError {
			kind,
			path: None,
			envelope: None,
			sequence_number: Some(sequence_number),
		})
	}

	pub fn cache_error_with_env(kind: CacheErrorKind, tx_envelope: TransactionEnvelope) -> Self {
		Self::CacheError(CacheError {
			kind,
			path: None,
			envelope: Some(tx_envelope),
			sequence_number: None,
		})
	}
}

#[derive(Error, PartialEq, Eq)]
pub struct CacheError {
	pub(crate) kind: CacheErrorKind,
	path: Option<String>,
	envelope: Option<TransactionEnvelope>,
	sequence_number: Option<SequenceNumber>,
}
impl Display for CacheError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl Debug for CacheError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		if let Some(env) = &self.envelope {
			return write!(f, "kind: {:?}, envelope: {:?}", self.kind, env)
		}

		if let Some(seq) = &self.sequence_number {
			return write!(f, "kind: {:?}, sequence_number: {}", self.kind, seq)
		}

		if let Some(path) = &self.path {
			return write!(f, "kind: {:?}, path: {}", self.kind, path)
		}

		write!(f, "kind: {:?}", self.kind)
	}
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CacheErrorKind {
	#[error("Could not extract sequence number from transaction")]
	UnknownSequenceNumber,

	#[error("Sequence number already in use.")]
	SequenceNumberAlreadyUsed,

	#[error("Failed to create directory")]
	CreateDirectoryFailed,

	#[error("Failed to create file")]
	FileCreationFailed,

	#[error("Failed to write file")]
	WriteToFileFailed,

	#[error("Failed to delete file")]
	DeleteFileFailed,

	#[error("File does not exist")]
	FileDoesNotExist,

	#[error("Failed to read file")]
	ReadFileFailed,

	#[error("File is invalid")]
	InvalidFile,

	#[error("Failed to decode file")]
	DecodeFileFailed,
}
