#![allow(dead_code)] //todo: remove after being tested and implemented

use crate::sdk::types::{
	AuthenticatedMessage, AuthenticatedMessageV0, HmacSha256Mac, MessageType, StellarMessage,
};
use std::fmt::Debug;
use substrate_stellar_sdk::XdrCodec;

#[derive(Debug, Eq, PartialEq, err_derive::Error)]
pub enum Error {
	#[error(display = "Data more than the max of U32")]
	UsizeToU32ExceedMax,

	#[error(display = "Message Version: Unsupported")]
	UnsupportedMessageVersion,

	#[error(display = "Decode Error: {}", _0)]
	DecodeError(String),
}

/// The 1st 4 bytes determines the byte length of the next stellar message.
/// Returns 0 if the array of u8 is less than 4 bytes, or exceeds the max of u32.
pub(crate) fn get_xdr_message_length(data: &[u8]) -> usize {
	if data.len() < 4 {
		return 0
	}

	let mut message_len = data[0..4].to_vec();
	message_len[0] &= 0x7f;

	let len = u32::from_be_bytes(message_len.try_into().unwrap_or([0, 0, 0, 0]));
	usize::try_from(len).unwrap_or(0)
}

/// Returns xdr of the authenticated message
pub(crate) fn from_authenticated_message(message: &AuthenticatedMessage) -> Result<Vec<u8>, Error> {
	message_to_bytes(message)
}

/// To easily convert any bytes to a Stellar type.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use substrate_stellar_sdk::types::Auth;
/// use stellar_relay_lib::parse_stellar_type;
/// let auth_xdr =  [0, 0, 0, 1];
/// let result = parse_stellar_type!(auth_xdr,Auth);
/// assert_eq!(result, Ok(Auth { flags: 1 }))
/// ```
#[macro_export]
macro_rules! parse_stellar_type {
	($ref:ident, $struct_str:ident) => {{
		use $crate::{
			sdk::{types::$struct_str, XdrCodec},
			xdr_converter::Error,
		};

		let ret: Result<$struct_str, Error> = $struct_str::from_xdr($ref).map_err(|e| {
			log::error!("decode error: {:?}", e);
			Error::DecodeError(stringify!($struct_str).to_string())
		});
		ret
	}};
}

/// Parses the xdr message into `AuthenticatedMessageV0`.
/// When successful, returns a tuple of the message and the `MessageType`.
pub(crate) fn parse_authenticated_message(
	xdr_message: &[u8],
) -> Result<(AuthenticatedMessageV0, MessageType), Error> {
	let xdr_msg_len = xdr_message.len();

	let msg_vers = parse_message_version(&xdr_message[0..4])?;
	if msg_vers != 0 {
		return Err(Error::UnsupportedMessageVersion)
	}

	let msg_type = parse_message_type(&xdr_message[12..16])?;

	Ok((
		AuthenticatedMessageV0 {
			sequence: parse_sequence(&xdr_message[4..12])?,
			message: parse_stellar_message(&xdr_message[12..(xdr_msg_len - 32)])?,
			mac: parse_hmac(&xdr_message[(xdr_msg_len - 32)..xdr_msg_len])?,
		},
		msg_type,
	))
}

fn parse_stellar_message(xdr_message: &[u8]) -> Result<StellarMessage, Error> {
	parse_stellar_type!(xdr_message, StellarMessage)
}

fn parse_message_version(xdr_message: &[u8]) -> Result<u32, Error> {
	u32::from_xdr(xdr_message).map_err(|e| log_decode_error("Message Version", e))
}

fn parse_sequence(xdr_message: &[u8]) -> Result<u64, Error> {
	u64::from_xdr(xdr_message).map_err(|e| log_decode_error("Sequence", e))
}

fn parse_hmac(xdr_message: &[u8]) -> Result<HmacSha256Mac, Error> {
	parse_stellar_type!(xdr_message, HmacSha256Mac)
}

fn parse_message_type(xdr_message: &[u8]) -> Result<MessageType, Error> {
	parse_stellar_type!(xdr_message, MessageType)
}

/// Returns XDR format of the message or
/// an error when the message length exceeds the max of u32
fn message_to_bytes<T: XdrCodec>(message: &T) -> Result<Vec<u8>, Error> {
	let mut message_xdr = message.to_xdr();

	// get the bytes of the message xdr's length.
	let message_len_bytes = u32::try_from(message_xdr.len())
		.map(|len| len.to_be_bytes())
		.map_err(|_| Error::UsizeToU32ExceedMax)?;

	let mut buffer: Vec<u8> = vec![];
	// first 4 bytes are for the length
	buffer.extend_from_slice(&message_len_bytes);
	// the message
	buffer.append(&mut message_xdr);

	Ok(buffer)
}

pub fn log_decode_error<T: Debug>(source: &str, error: T) -> Error {
	log::error!("decode error: {:?}", error);
	Error::DecodeError(source.to_string())
}

// extra function.
fn is_xdr_complete_message(data: &[u8], message_len: usize) -> bool {
	return data.len() - 4 >= message_len
}

// extra function
fn get_message(data: &[u8], message_len: usize) -> (Vec<u8>, Vec<u8>) {
	(data[4..(message_len + 4)].to_owned(), data[0..(message_len + 4)].to_owned())
}

#[cfg(test)]
mod test {
	use crate::connection::xdr_converter::{
		get_message, get_xdr_message_length, is_xdr_complete_message, parse_authenticated_message,
	};

	#[test]
	fn get_xdr_message_length_success() {
		let arr: [u8; 4] = [128, 0, 1, 28];

		assert_eq!(get_xdr_message_length(&arr), 284);
	}

	#[test]
	fn parse_authenticated_message_success() {
		let msg = base64::decode_config(
            "AAAAAAAAAAAAAAA7AAAACwAAAACMHUtKNgEX1QDfz4zesWaxmhLg9Le806GgxemeQfaXmQAAAAACKDOuAAAAAzQaCq4p6tLHpdfwGhnlyX9dMUP70r4Dm98Td6YvKnhoAAAAAQAAAJg1D82tsvx59BI2BldZq12xYzdrhUkIflWnRwbiJsoMUgAAAABg4A0jAAAAAAAAAAEAAAAAUwoi9HcvJrwUn5w15omNdNffAJKoHHDdZh+2c+8VUd4AAABAB5/NoeG4iJJitcTDJvdhDLaLL9FSUHodRXvMEjbGKeDSkSXDgl+q+VvDXenwQNOOhLg112bsviGwh61ci4HnAgAAAAEAAACYNQ/NrbL8efQSNgZXWatdsWM3a4VJCH5Vp0cG4ibKDFIAAAAAYOANIwAAAAAAAAABAAAAAFMKIvR3Lya8FJ+cNeaJjXTX3wCSqBxw3WYftnPvFVHeAAAAQAefzaHhuIiSYrXEwyb3YQy2iy/RUlB6HUV7zBI2xing0pElw4Jfqvlbw13p8EDTjoS4Nddm7L4hsIetXIuB5wIAAABAyN92d7osuHXtUWHoEQzSRH5f9h6oEQAGK02b4CO4bQchmpbwbqGQLdbD9psFpamuLrDK+QJiBuKw3PVnMNlMDA9Ws6xvU3NyJ/OBsg2EZicl61zCYxrQXQ4Qq/eXI+wT",
            base64::STANDARD
        ).expect("should be able to decode to bytes");

		//todo: once the authenticatedmessagev0 type is solved, continue the test
		let _ = parse_authenticated_message(&msg);
	}

	#[test]
	fn message_not_complete_check() {
		let xdr_no_next_msg = base64::decode_config(
            "gAABaAAAAAAAAAAAAAAAAgAAAAsAAAAAAsUlnka7dHFfp69mUW6kEQ18IpsXLwcYk6yphpesUysAAAAAAULT7wAAAAN1tE4FkHboorc8QsJU7+LkIN2zbNK9MrkY49OpVcEzDwAAAAIAAAAw/0TiDQ==",
            base64::STANDARD
        ).expect("should be able to decode to bytes");

		let len = get_xdr_message_length(&xdr_no_next_msg);
		assert!(!is_xdr_complete_message(&xdr_no_next_msg, len));
	}

	#[test]
	fn message_complete() {
		let xdr_has_next_msg = base64::decode_config(
            "gAAANAAAAAAAAAAAAAAAAAAAAAIAAAAAv2qE3dixC3UHHZmFXQGPliZ90ghxAiO5C4fYG/G4EeqAAANUAAAAAAAAAAAAAAABAAAABQAAADIAAAAAqTm9CAAALWkAAAAAAAAAAKkvb34AAC1pAAAAAAAAAAA23YxJAAAtaQAAAAAAAAAANDdVmQAALWkAAAAAAAAAAKkzSDUAAC1pAAAAAAAAAACer1MIAAAtaQAAAAAAAAAAI8ZHBAAALWkAAAAAAAAAACPGQFcAAC1pAAAAAAAAAACCxkWYAAAtaQAAAAAAAAAAaxSf6AAALWkAAAAAAAAAACv/s4IAAC1pAAAAAAAAAAA2TpjFAAAtaQAAAAAAAAAAsj7l8gAALWkAAAAAAAAAALhIZ40AAC1pAAAAAAAAAACeQEz9AAAtaQAAAAAAAAAAq2DFJwAALWkAAAAAAAAAADZKcDEAAC1pAAAAAAAAAAAnPPzbAAAtaQAAAAAAAAAAcyEZJgAALWkAAAAAAAAAACm+Dn4AAC1pAAAAAAAAAAB04vMyAAAtaQAAAAAAAAAANk4tVQAALWkAAAAAAAAAANRcdnwAAC1pAAAAAAAAAAB9J5EIAAAtaQAAAAAAAAAApeOhygAALWkAAAAAAAAAAK4kOPYAAC1pAAAAAAAAAABnC1mkAAAtaQAAAAAAAAAANqo7KwAALWkAAAAAAAAAADRO0w0AAC1pAAAAAAAAAAABtNQAAAAtaQAAAAAAAAAAcm+nvgAALWkAAAAAAAAAALS/SCYAAC1pAAAAAAAAAAA2SvPwAAAtaQAAAAAAAAAANpKzmAAALWkAAAAAAAAAAJ5VSogAAC1pAAAAAAAAAACyotgHAAAtaQAAAAAAAAAANkoQ2wAALWkAAAAAAAAAADZOAFMAAC1pAAAAAAAAAAA2qm9GAAAtaQAAAAAAAAAANpvTAgAALWkAAAAAAAAAAG/GQiEAAC1pAAAAAAAAAAAju7lmAAAtaQAAAAAAAAAANk7rHwAALWkAAAAAAAAAABfyLQ0AAC1pAAAAAAAAAAA2TjaRAAAtaQAAAAAAAAAANk5J7gAALWkAAAAAAAAAAHRm84QAAC1pAAAAAAAAAAAr/7KvAAAtaQAAAAAAAAAAufEGsgAALWkAAAAAAAAAADZOzt4AAC1pAAAAADKjiRSHBdyaVK1C+7UoMAGGyLJ5D1CjOi7gsns2GEFBgAABaAAAAAAAAAAAAAAAAgAAAAsAAAAAAsUlnka7dHFfp69mUW6kEQ18IpsXLwcYk6yphpesUysAAAAAAULT7wAAAAN1tE4FkHboorc8QsJU7+LkIN2zbNK9MrkY49OpVcEzDwAAAAIAAAAw/0TiDQ==",
            base64::STANDARD
        ).expect("should be able to decode to bytes");

		let len = get_xdr_message_length(&xdr_has_next_msg);
		let (nxt_msg, _) = get_message(&xdr_has_next_msg, len);

		let str = base64::encode(nxt_msg);
		assert_eq!(
			&str,
			"AAAAAAAAAAAAAAAAAAAAAgAAAAC/aoTd2LELdQcdmYVdAY+WJn3SCHECI7kLh9gb8bgR6g=="
		);
	}
}
