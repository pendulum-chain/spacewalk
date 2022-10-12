#![allow(dead_code)] //todo: remove after being tested and implemented

use crate::connection::xdr_converter::Error as XDRError;
use substrate_stellar_sdk::StellarSdkError;
use tokio::sync;

#[derive(Debug, err_derive::Error)]
pub enum Error {
    #[error(display = "Authentication Certification: Expired")]
    AuthCertExpired,

    #[error(display = "Authentication Certification: Not Found")]
    AuthCertNotFound,

    #[error(display = "Authentication Certification: Invalid")]
    AuthCertInvalid,

    #[error(display = "Connection: {}", _0)]
    ConnectionFailed(String),

    #[error(display = "Write: {}", _0)]
    WriteFailed(String),

    #[error(display = "Sent: {}", _0)]
    SendFailed(String),

    #[error(display = "Read: {}", _0)]
    ReadFailed(String),

    #[error(display = "The Remote Node Info wasn't initialized.")]
    NoRemoteInfo,

    #[error(display = "Sequence num with the Auth message is different with remote sequence")]
    InvalidSequenceNumber,

    #[error(display = "Hmac: {:?}", _0)]
    HmacError(hmac::digest::MacError),

    #[error(display = "The Hmac Key is ivalide")]
    MissingHmacKeys,

    #[error(display = "Hmac: Invalid Length")]
    HmacInvalidLength,

    #[error(display = "{:?}", _0)]
    XDRConversionError(XDRError),

    #[error(display = "{:?}", _0)]
    StellarSdkError(StellarSdkError),
}

impl From<XDRError> for Error {
    fn from(e: XDRError) -> Self {
        Error::XDRConversionError(e)
    }
}

impl<T> From<sync::mpsc::error::SendError<T>> for Error {
    fn from(e: sync::mpsc::error::SendError<T>) -> Self {
        Error::SendFailed(e.to_string())
    }
}

impl From<StellarSdkError> for Error {
    fn from(e: StellarSdkError) -> Self {
        Error::StellarSdkError(e)
    }
}
