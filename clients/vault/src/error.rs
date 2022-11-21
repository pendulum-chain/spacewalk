use hex::FromHexError;
use jsonrpc_core_client::RpcError;
use parity_scale_codec::Error as CodecError;
use sp_runtime::traits::LookupError;
use sp_std::str::Utf8Error;
use thiserror::Error;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use runtime::{Error as RuntimeError, SubxtError};
use service::Error as ServiceError;
use stellar_relay::sdk::StellarSdkError;
use wallet::Error as WalletError;

#[derive(Error, Debug)]
pub enum Error {
	#[error("Insufficient funds available")]
	InsufficientFunds,
	#[error("Mathematical operation caused an overflow")]
	ArithmeticOverflow,
	#[error("Mathematical operation caused an underflow")]
	ArithmeticUnderflow,
	#[error(transparent)]
	TryIntoIntError(#[from] std::num::TryFromIntError),
	#[error("Deadline has expired")]
	DeadlineExpired,
	#[error("Faucet url not set")]
	FaucetUrlNotSet,

	#[error("RPC error: {0}")]
	RpcError(#[from] RpcError),
	#[error("RuntimeError: {0}")]
	RuntimeError(#[from] RuntimeError),
	#[error("CodecError: {0}")]
	CodecError(#[from] CodecError),
	#[error("BroadcastStreamRecvError: {0}")]
	BroadcastStreamRecvError(#[from] BroadcastStreamRecvError),
	#[error("StellarWalletError: {0}")]
	StellarWalletError(#[from] WalletError),

	#[error("Lookup Error")]
	LookupError,
	#[error("Stellar SDK Error")]
	StellarSdkError,
}

impl From<Error> for service::Error<Error> {
	fn from(err: Error) -> Self {
		Self::Retry(err)
	}
}
