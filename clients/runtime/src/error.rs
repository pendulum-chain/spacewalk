use std::{array::TryFromSliceError, fmt::Debug, io::Error as IoError, num::TryFromIntError};

use codec::Error as CodecError;
pub use jsonrpsee::core::Error as JsonRpseeError;
use jsonrpsee::{client_transport::ws::WsHandshakeError, types::ErrorObjectOwned};
use serde_json::Error as SerdeJsonError;
pub use subxt::{error::RpcError, Error as SubxtError};
use subxt::{
	error::{DispatchError, TransactionError},
	ext::sp_core::crypto::SecretStringError,
};
use thiserror::Error;
use tokio::time::error::Elapsed;

use crate::{types::*, ISSUE_MODULE, SECURITY_MODULE};
use prometheus::Error as PrometheusError;

#[derive(Error, Debug)]
pub enum Error {
	#[error("Could not get exchange rate info")]
	ExchangeRateInfo,
	#[error("The list is empty. At least one element is required.")]
	FeedingEmptyList,
	#[error("Could not get issue id")]
	RequestIssueIDNotFound,
	#[error("Could not get redeem id")]
	RequestRedeemIDNotFound,
	#[error("Could not get replace id")]
	RequestReplaceIDNotFound,
	#[error("Could not get block")]
	BlockNotFound,
	#[error("Could not get vault")]
	VaultNotFound,
	#[error("Vault has been liquidated")]
	VaultLiquidated,
	#[error("Vault has stolen Stellar assets")]
	VaultCommittedTheft,
	#[error("Channel closed unexpectedly")]
	ChannelClosed,
	#[error("Cannot replace existing transaction")]
	PoolTooLowPriority,
	#[error("Transaction did not get included - block hash not found")]
	BlockHashNotFound,
	#[error("Transaction is invalid: {0}")]
	InvalidTransaction(String),
	#[error("Request has timed out")]
	Timeout,
	#[error("Block is not in the relay main chain")]
	BlockNotInRelayMainChain,
	#[error("Invalid currency")]
	InvalidCurrency,
	#[error("Invalid keyring arguments")]
	KeyringArgumentError,
	#[error("Failed to parse keyring account")]
	KeyringAccountParsingError,
	#[error("Storage item not found")]
	StorageItemNotFound,
	#[error("Insufficient funds")]
	InsufficientFunds,
	#[error("Client does not support spec_version: expected {0}..={1}, got {2}")]
	InvalidSpecVersion(u32, u32, u32),
	#[error("Client metadata is different from parachain metadata: expected {0}, got {1}")]
	ParachainMetadataMismatch(String, String),
	#[error("Failed to load credentials from file: {0}")]
	KeyLoadingFailure(#[from] KeyLoadingError),
	#[error("Error serializing: {0}")]
	Serialize(#[from] TryFromSliceError),
	#[error("Error converting: {0}")]
	Convert(#[from] TryFromIntError),
	#[error("Subxt runtime error: {0}")]
	SubxtRuntimeError(#[from] SubxtError),
	#[error("Error decoding: {0}")]
	CodecError(#[from] CodecError),
	#[error("Error encoding json data: {0}")]
	SerdeJsonError(#[from] SerdeJsonError),
	#[error("Error getting json-rpsee data: {0}")]
	JsonRpseeError(#[from] JsonRpseeError),
	#[error("Timeout: {0}")]
	TimeElapsed(#[from] Elapsed),
	#[error("UrlParseError: {0}")]
	UrlParseError(#[from] url::ParseError),
	#[error("Constant not found: {0}")]
	ConstantNotFound(String),
	#[error("Currency not found")]
	CurrencyNotFound,
	#[error("PrometheusError: {0}")]
	PrometheusError(#[from] PrometheusError),
}

pub enum Recoverability {
	Recoverable(String),
	Unrecoverable(String),
}

enum RecoverableError {
	InabilityToPayFees,
	OutdatedTransaction,
}

impl Error {
	pub fn is_module_err(&self, pallet_name: &str, error_name: &str) -> bool {
		matches!(
			self,
			Error::SubxtRuntimeError(SubxtError::Runtime(DispatchError::Module(module_error))) if {
				match module_error.details() {
					Ok(details) => details.pallet.name() == pallet_name && &details.variant.name == error_name,
					Err(_) => false
				}
			},
		)
	}

	pub fn is_issue_completed(&self) -> bool {
		self.is_module_err(ISSUE_MODULE, &format!("{:?}", IssuePalletError::IssueCompleted))
	}

	fn map_custom_error<T>(&self, call: impl Fn(&ErrorObjectOwned) -> Option<T>) -> Option<T> {
		if let Error::SubxtRuntimeError(SubxtError::Rpc(RpcError::ClientError(e))) = self {
			match e.downcast_ref::<JsonRpseeError>() {
				Some(e) => match e {
					JsonRpseeError::Call(err) => call(err),
					_ => None,
				},
				None => {
					log::error!("Failed to downcast RPC error; this is a bug please file an issue");
					None
				},
			}
		} else {
			None
		}
	}

	pub fn is_invalid_transaction(&self) -> Option<Recoverability> {
		self.map_custom_error(|custom_error| {
			if custom_error.code() == POOL_INVALID_TX {
				let data_string = custom_error.data().map(ToString::to_string).unwrap_or_default();

				if RecoverableError::is_recoverable(&data_string) {
					return Some(Recoverability::Recoverable(data_string));
				}

				return Some(Recoverability::Unrecoverable(data_string));
			} else {
				None
			}
		})
	}

	pub fn is_pool_too_low_priority(&self) -> bool {
		self.map_custom_error(|custom_error| {
			if custom_error.code() == POOL_TOO_LOW_PRIORITY {
				Some(())
			} else {
				None
			}
		})
		.is_some()
	}

	pub fn is_rpc_disconnect_error(&self) -> bool {
		match self {
			Error::SubxtRuntimeError(SubxtError::Rpc(RpcError::ClientError(e))) =>
				match e.downcast_ref::<JsonRpseeError>() {
					Some(e) => matches!(e, JsonRpseeError::RestartNeeded(_)),
					None => {
						log::error!(
							"Failed to downcast RPC error; this is a bug please file an issue"
						);
						false
					},
				},
			Error::SubxtRuntimeError(SubxtError::Rpc(RpcError::SubscriptionDropped)) => true,
			_ => false,
		}
	}

	pub fn is_rpc_error(&self) -> bool {
		matches!(self, Error::SubxtRuntimeError(SubxtError::Rpc(_)))
	}

	pub fn is_block_hash_not_found_error(&self) -> bool {
		matches!(
			self,
			Error::SubxtRuntimeError(SubxtError::Transaction(TransactionError::BlockNotFound))
		)
	}

	pub fn is_ws_invalid_url_error(&self) -> bool {
		matches!(
			self,
			Error::JsonRpseeError(JsonRpseeError::Transport(err))
			if matches!(err.downcast_ref::<WsHandshakeError>(), Some(WsHandshakeError::Url(_)))
		)
	}

	pub fn is_parachain_shutdown_error(&self) -> bool {
		self.is_module_err(
			SECURITY_MODULE,
			&format!("{:?}", SecurityPalletError::ParachainNotRunning),
		)
	}

	pub fn is_timeout_error(&self) -> bool {
		match self {
			Error::SubxtRuntimeError(SubxtError::Rpc(RpcError::ClientError(e))) =>
				match e.downcast_ref::<JsonRpseeError>() {
					Some(e) => matches!(e, JsonRpseeError::RequestTimeout),
					None => {
						log::error!(
							"Failed to downcast RPC error; this is a bug please file an issue"
						);
						false
					},
				},
			_ => false,
		}
	}
}

impl RecoverableError {
	// For definitions, see: https://github.com/paritytech/substrate/blob/033d4e86cc7eff0066cd376b9375f815761d653c/primitives/runtime/src/transaction_validity.rs#L99

	fn as_str(&self) -> &'static str {
		match self {
			RecoverableError::InabilityToPayFees => "Inability to pay some fees",
			RecoverableError::OutdatedTransaction => "Transaction is outdated",
		}
	}

	fn is_recoverable(error_message: &str) -> bool {
		[RecoverableError::InabilityToPayFees, RecoverableError::OutdatedTransaction]
			.iter()
			.any(|error| error_message.contains(error.as_str()))
	}
}

#[derive(Error, Debug)]
pub enum KeyLoadingError {
	#[error("Key not found in file")]
	KeyNotFound,
	#[error("Json parsing error: {0}")]
	JsonError(#[from] SerdeJsonError),
	#[error("Io error: {0}")]
	IoError(#[from] IoError),
	#[error("Invalid secret string: {0:?}")]
	SecretStringError(SecretStringError),
}

// https://github.com/paritytech/substrate/blob/e60597dff0aa7ffad623be2cc6edd94c7dc51edd/client/rpc-api/src/author/error.rs#L80
const BASE_ERROR: i32 = 1000;
const POOL_INVALID_TX: i32 = BASE_ERROR + 10;
const POOL_TOO_LOW_PRIORITY: i32 = POOL_INVALID_TX + 4;
