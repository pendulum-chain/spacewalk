use crate::types::StatusCode;
use primitives::stellar::{types::SequenceNumber, TransactionEnvelope};
use reqwest::Error as ReqwestError;
use std::fmt::{Debug, Display, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
	#[error("Invalid secret key")]
	InvalidSecretKey,
	#[error("Error fetching horizon data: error: {error:?}, other: {other:?}")]
	HorizonResponseError { error: Option<ReqwestError>, status: Option<u16>, other: Option<String> },
	#[error("Could not build transaction: {0}")]
	BuildTransactionError(String),
	#[error("Transaction submission failed. Title: {title}, Status: {status}, Reason: {reason}, Envelope XDR: {envelope_xdr:?}")]
	HorizonSubmissionError {
		title: String,
		status: StatusCode,
		reason: String,
		result_code_op: Vec<String>,
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

	#[error("Transaction resubmission failed: {0}")]
	ResubmissionError(String),

	#[error("Cannot send payment to self")]
	SelfPaymentError,

	#[error("Failed to get fee: {0}")]
	FailedToGetFee(String),
}

impl Error {
	pub fn is_recoverable(&self) -> bool {
		match self {
			Error::HorizonResponseError { status, error, .. } => {
				if let Some(e) = error {
					if e.is_timeout() {
						return true;
					}
				}

				if let Some(status) = status {
					// forbidden error
					if *status == 403 {
						return true;
					}
				}

				false
			},
			Error::HorizonSubmissionError { status, .. } if *status == 504 => true,
			Error::CacheError(e) => match e.kind {
				CacheErrorKind::CreateDirectoryFailed |
				CacheErrorKind::FileCreationFailed |
				CacheErrorKind::WriteToFileFailed |
				CacheErrorKind::DeleteFileFailed |
				CacheErrorKind::FileDoesNotExist => true,
				_ => false,
			},
			_ => false,
		}
	}

	pub fn is_server_error(&self) -> bool {
		let server_errors = 500u16..599;

		match self {
			Error::HorizonResponseError { status, error, .. } => {
				if let Some(e) = error {
					return e
						.status()
						.map(|code| server_errors.contains(&code.as_u16()))
						.unwrap_or(false);
				}

				if let Some(status) = status {
					return server_errors.contains(status);
				}

				// by default, assume that it will be a client error.
				false
			},
			Error::HorizonSubmissionError { status, .. } => server_errors.contains(status),
			_ => false,
		}
	}

	pub fn response_decode_error(status: StatusCode, response_in_bytes: &[u8]) -> Self {
		let resp_as_str = std::str::from_utf8(response_in_bytes).map(|s| s.to_string()).ok();
		Error::HorizonResponseError { error: None, status: Some(status), other: resp_as_str }
	}

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
	pub(crate) path: Option<String>,
	pub(crate) envelope: Option<TransactionEnvelope>,
	pub(crate) sequence_number: Option<SequenceNumber>,
}
impl Display for CacheError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl Debug for CacheError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		if let Some(env) = &self.envelope {
			return write!(f, "kind: {:?}, envelope: {:?}", self.kind, env);
		}

		if let Some(seq) = &self.sequence_number {
			return write!(f, "kind: {:?}, sequence_number: {}", self.kind, seq);
		}

		if let Some(path) = &self.path {
			return write!(f, "kind: {:?}, path: {}", self.kind, path);
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
