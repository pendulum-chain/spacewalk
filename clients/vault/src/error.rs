use hex::FromHexError;
use jsonrpc_core_client::RpcError;
use parity_scale_codec::Error as CodecError;
use runtime::{Error as RuntimeError, SubxtError};
use service::Error as ServiceError;
use sp_runtime::traits::LookupError;
use sp_std::str::Utf8Error;
use stellar_relay_lib::sdk::StellarSdkError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("Insufficient funds available")]
	InsufficientFunds,
	#[error("Value below dust amount")]
	BelowDustAmount,
	#[error("Transaction contains more than one return-to-self uxto")]
	TooManyReturnToSelfAddresses,
	#[error("Mathematical operation caused an overflow")]
	ArithmeticOverflow,
	#[error("Mathematical operation caused an underflow")]
	ArithmeticUnderflow,
	#[error(transparent)]
	TryIntoIntError(#[from] std::num::TryFromIntError),
	#[error("Deadline has expired")]
	DeadlineExpired,

	#[error("ServiceError: {0}")]
	ServiceError(#[from] ServiceError),
	#[error("RPC error: {0}")]
	RpcError(#[from] RpcError),
	#[error("Hex conversion error: {0}")]
	FromHexError(#[from] FromHexError),
	#[error("RuntimeError: {0}")]
	RuntimeError(#[from] RuntimeError),
	#[error("SubxtError: {0}")]
	SubxtError(#[from] SubxtError),
	#[error("CodecError: {0}")]
	CodecError(#[from] CodecError),

	#[error("Error returned when fetching remote info")]
	HttpFetchingError,
	#[error("Failed to post http request")]
	HttpPostError,
	#[error("Lookup Error")]
	LookupError,
	#[error("Stellar SDK Error")]
	StellarSdkError,
	#[error("ExceedsMaximumLengthError")]
	ExceedsMaximumLengthError,
	#[error("Failed to convert balance")]
	BalanceConversionError,
	#[error("Utf8Error: {0}")]
	Utf8Error(#[from] Utf8Error),
	#[error("Failed to parse sequence number")]
	SeqNoParsingError,
}

impl From<StellarSdkError> for Error {
	fn from(error: StellarSdkError) -> Self {
		match error {
			StellarSdkError::ExceedsMaximumLength { .. } => Self::ExceedsMaximumLengthError,
			_ => Self::StellarSdkError,
		}
	}
}

impl From<LookupError> for Error {
	fn from(_: LookupError) -> Self {
		Self::BalanceConversionError
	}
}
