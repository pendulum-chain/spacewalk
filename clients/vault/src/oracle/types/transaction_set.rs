use stellar_relay_lib::sdk::{
	types::{GeneralizedTransactionSet, TransactionSet},
	XdrCodec,
};

use crate::oracle::Error;

pub type Base64EncodedTxSet = String;
pub type Base64EncodedTxSetForMapping = String;

pub trait TxSetBase64Codec: XdrCodec + Sized {
	const XDR_PREFIX: u8;
	const DECODE_ERROR_TITLE: &'static str;
	/// returns a base64 encoded xdr string
	///
	/// first gets the xdr version of the object
	/// adds a prefix u8 value on that xdr `Vec<u8>`
	///     * 0 for `TxSet`,
	///     * 1 for `GeneralizedTxSet`
	/// encode it to base64
	fn to_base64_encoded_xdr_string_for_mapping(&self) -> Base64EncodedTxSetForMapping {
		let mut final_txset_xdr = vec![];
		// we prepend a prefix value to this encoded data to indicate its type.
		final_txset_xdr.push(Self::XDR_PREFIX);

		let mut txset_xdr = self.to_xdr();
		final_txset_xdr.append(&mut txset_xdr);

		base64::encode(final_txset_xdr)
	}

	/// decodes a base64 string
	/// removes the prefix value of the xdr
	/// and then converts to either `TxSet` or `GeneralizedTxSet`
	fn from_base64_encoded_xdr_string_for_mapping(
		base64_encoded_tx_set: &Base64EncodedTxSetForMapping,
	) -> Result<Self, Error> {
		let mut final_txset_xdr = base64::decode(base64_encoded_tx_set)
			.map_err(|e| Error::DecodeError(format!("{}{e:?}", Self::DECODE_ERROR_TITLE)))?;

		if final_txset_xdr.remove(0) != Self::XDR_PREFIX {
			return Err(Error::DecodeError(format!(
				"{}Prefix is not {} inside this encoded set: {base64_encoded_tx_set}",
				Self::DECODE_ERROR_TITLE,
				Self::XDR_PREFIX
			)))
		}

		Self::from_xdr(final_txset_xdr)
			.map_err(|e| Error::DecodeError(format!("{}{e:?}", Self::DECODE_ERROR_TITLE)))
	}
}

impl TxSetBase64Codec for GeneralizedTransactionSet {
	const XDR_PREFIX: u8 = 1;
	const DECODE_ERROR_TITLE: &'static str = "Cannot decode to GeneralizedTxSet: ";
}

impl TxSetBase64Codec for TransactionSet {
	const XDR_PREFIX: u8 = 0;
	const DECODE_ERROR_TITLE: &'static str = "Cannot decode to TxSet: ";
}

#[cfg(test)]
pub mod tests {
	use super::TxSetBase64Codec;
	use crate::oracle::Error;
	use std::{env, fs::File, io::Read, path::PathBuf};
	use stellar_relay_lib::sdk::{
		types::{GeneralizedTransactionSet, TransactionSet},
		XdrCodec,
	};

	fn open_file(file_name: &str) -> Vec<u8> {
		let x = env::current_dir();

		let mut path = PathBuf::new();
		let path_str = format!("./resources/samples/{file_name}");
		path.push(&path_str);

		let mut file = File::open(path).expect("file should exist");
		let mut bytes: Vec<u8> = vec![];
		let _ = file.read_to_end(&mut bytes).expect("should be able to read until the end");

		bytes
	}

	fn sample_gen_txset_as_vec_u8() -> Vec<u8> {
		open_file("generalized_tx_set")
	}

	fn sample_txset_as_vec_u8() -> Vec<u8> {
		open_file("tx_set")
	}

	pub fn sample_gen_txset() -> GeneralizedTransactionSet {
		let sample = sample_gen_txset_as_vec_u8();

		GeneralizedTransactionSet::from_base64_xdr(sample)
			.expect("should return a generalized transaction set")
	}

	pub fn sample_txset() -> TransactionSet {
		let sample = sample_txset_as_vec_u8();

		TransactionSet::from_base64_xdr(sample).expect("should return a transaction set")
	}

	fn _encode_decode_test_for_success<T>(bytes: Vec<u8>) -> T
	where
		T: XdrCodec + TxSetBase64Codec,
	{
		let txset = T::from_base64_xdr(&bytes).expect("should convert successfully");

		let txset_as_string = std::str::from_utf8(&bytes)
			.expect("should return a string format of the txset/gen txset");

		let txset_as_string_for_mapping = txset.to_base64_encoded_xdr_string_for_mapping();

		// these strings should not be the same; the other one has an additional prefix which is
		// used for mapping
		assert_ne!(txset_as_string_for_mapping, txset_as_string);

		// this string is only used for mapping. Cannot generate a txset/gen txset from a modified
		// xdr
		assert!(
			T::from_base64_encoded_xdr_string_for_mapping(&txset_as_string.to_string()).is_err()
		);

		// decodes the modified xdr
		let txset_new = T::from_base64_encoded_xdr_string_for_mapping(&txset_as_string_for_mapping)
			.expect("should be able to return a Gen txset");

		assert_eq!(txset_new.to_xdr(), txset.to_xdr());

		txset_new
	}

	#[test]
	fn encode_decode_test_for_success() {
		_encode_decode_test_for_success::<TransactionSet>(sample_txset_as_vec_u8());

		let gen_txset_from_file = sample_gen_txset_as_vec_u8();
		let gen_txset_FROM_MAPPING = _encode_decode_test_for_success::<GeneralizedTransactionSet>(
			gen_txset_from_file.clone(),
		);

		// test the `fn to_base64_encoded_xdr_string` of `GeneralizedTxSet`
		let gen_txset_str_FROM_MAPPING = gen_txset_FROM_MAPPING
			.to_base64_encoded_xdr_string()
			.expect("should return a string WITHOUT THE PREFIX");

		assert_eq!(
			gen_txset_str_FROM_MAPPING,
			String::from_utf8(gen_txset_from_file).expect("should be able to convert to string")
		)
	}

	#[test]
	fn encode_decode_test_for_failures() {
		fn convert<T1: XdrCodec + TxSetBase64Codec, T2: TxSetBase64Codec>(bytes: Vec<u8>) {
			let txset = T1::from_base64_xdr(&bytes).expect("should convert successfully");

			let txset_string_for_mapping = txset.to_base64_encoded_xdr_string_for_mapping();

			// error happens when trying to convert a gen txset to txset and vice versa
			assert!(
				T2::from_base64_encoded_xdr_string_for_mapping(&txset_string_for_mapping).is_err()
			);
		}

		convert::<TransactionSet, GeneralizedTransactionSet>(sample_txset_as_vec_u8());
		convert::<GeneralizedTransactionSet, TransactionSet>(sample_gen_txset_as_vec_u8());
	}
}
