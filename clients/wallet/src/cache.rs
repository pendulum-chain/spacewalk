use primitives::TransactionEnvelopeExt;
use std::{
	fs::{create_dir_all, read_dir, remove_file, File, OpenOptions},
	io::{Read, Write},
	path::Path,
};
use substrate_stellar_sdk::{types::SequenceNumber, TransactionEnvelope, XdrCodec};

/// a helpful macro to unwrap an `Ok` or return immediately.
macro_rules! unwrap_or_return {
	// expression, return value, extra log
	($res:expr, $ret:expr, $log:expr) => {
		match $res {
			Ok(result) => result,
			Err(e) => {
				tracing::warn!("{:?}: {:?}", $log, e);
				return $ret
			},
		}
	};
	($res:expr, $ret:expr) => {
		match $res {
			Ok(result) => result,
			Err(e) => {
				tracing::warn!("{:?}", e);
				return $ret
			},
		}
	};
	($res:expr) => {
		match $res {
			Ok(result) => result,
			Err(e) => {
				tracing::warn!("{:?}", e);
				return
			},
		}
	};
}

/// Contains the path where the transaction envelope will be saved.
#[derive(Debug, Clone)]
pub struct TxEnvelopeStorage {
	path: String,
	inner_path: String,
}

impl TxEnvelopeStorage {
	/// returns the full directory path
	fn full_path(&self) -> String {
		format!("{}/{}", self.path, self.inner_path)
	}
}

impl TxEnvelopeStorage {
	pub fn new(path: String, public_key: &str, is_public_network: bool) -> Self {
		let inner_path = format!("{public_key}_{is_public_network}");
		let full_path = format!("{path}/{public_key}_{is_public_network}");

		if let Err(e) = create_dir_all(&full_path) {
			tracing::warn!("Failed to create directory of {full_path}: {:?}", e);
		}
		tracing::info!("path for caching stellar transactions: {full_path}");

		TxEnvelopeStorage { path, inner_path }
	}

	/// Returns true if transaction was saved to a local path.
	/// The filename will be the transaction's sequence number.
	pub fn save_to_local(
		&self,
		sequence: SequenceNumber,
		transaction: TransactionEnvelope,
	) -> bool {
		let full_file_path = format!("{}/{}", self.full_path(), sequence);

		let path = Path::new(&full_file_path);
		if path.exists() {
			return false
		}

		let mut file = unwrap_or_return!(
			OpenOptions::new().write(true).create(true).open(path),
			false,
			format!("Failed to create file for transaction with sequence {}", sequence)
		);

		unwrap_or_return!(
			// writing the transaction to file in Vec<u8> format
			write!(file, "{:?}", transaction.to_xdr()),
			false,
			format!("Failed to write transaction with sequence {}", sequence)
		);

		true
	}

	pub fn remove_transaction(&self, sequence: SequenceNumber) -> bool {
		let full_file_path = format!("{}/{sequence}", self.full_path());
		remove_file(full_file_path).is_ok()
	}

	#[doc(hidden)]
	#[cfg(any(test, feature = "testing-utils"))]
	/// Removes all transactions, including the directory
	pub fn remove_all(&self) -> bool {
		std::fs::remove_dir_all(self.full_path()).is_ok()
	}

	#[allow(dead_code)]
	#[cfg(any(test, feature = "testing-utils"))]
	/// Returns a transaction if a file was found, given the sequence number
	pub fn get_tx_envelope(&self, sequence: SequenceNumber) -> Option<TransactionEnvelope> {
		let full_file_path = format!("{}/{sequence}", self.full_path());
		let path = Path::new(&full_file_path);

		if !path.exists() {
			return None
		}
		read_tx_envelope_from_path(path).map(|(tx, _)| tx)
	}

	/// Returns all transactions found in local path
	pub fn get_tx_envelopes(&self) -> Vec<(TransactionEnvelope, SequenceNumber)> {
		let path = self.full_path();
		let directory =
			unwrap_or_return!(read_dir(&path), vec![], format!("Failed to read directory {path}"));

		let mut res: Vec<(TransactionEnvelope, SequenceNumber)> = directory
			.into_iter()
			.filter_map(|entry| {
				let path = entry.ok()?.path();
				extract_tx_envelope_from_path(path)
			})
			.collect();

		// the envelopes have been filtered out with those having sequence numbers already
		res.sort_by(|(_, first), (_, next)| first.cmp(&next));

		res
	}
}

/// a helper function to convert a String content into `Vec<u8>`
fn parse_string_to_vec_u8(value: &str) -> Option<Vec<u8>> {
	let remove_white_space = value.replace(' ', "");
	let remove_brackets = &remove_white_space[1..remove_white_space.len() - 1];
	let split_by_comma = remove_brackets.split(',');

	split_by_comma
		.enumerate()
		.map(|(pos, num_as_char)| {
			// parse this character to u8
			Some(unwrap_or_return!(
				num_as_char.parse(),
				None,
				format!("Invalid xdr string: {num_as_char} at pos: {pos}")
			))
		})
		.collect()
}

/// Returns a tuple of `TransactionEnvelope` and its sequence number,
/// if successfully extracted from the path.
/// Otherwise returns a None and and the file will be deleted.
fn extract_tx_envelope_from_path<P: AsRef<Path> + std::fmt::Debug + Clone>(
	path: P,
) -> Option<(TransactionEnvelope, SequenceNumber)> {
	let Some(env) = read_tx_envelope_from_path(&path) else {
		// do not remove invalid files for unit-testing.
		#[cfg(not(test))]
		if !remove_file(&path).is_ok() {
			tracing::warn!("failed to remove unreadable file: {:?}",path);
		}

		return None;
	};

	Some(env)
}

/// a helper function to parse a content of a file into `TransactionEnvelope`.
fn read_tx_envelope_from_path<P: AsRef<Path> + std::fmt::Debug + Clone>(
	path: P,
) -> Option<(TransactionEnvelope, SequenceNumber)> {
	let content_from_file = unwrap_or_return!(
		OpenOptions::new().read(true).open(&path).and_then(|mut file: File| {
			// if the file exists, read the contents as a string.
			let mut buf = String::new();

			if let Err(e) = file.read_to_string(&mut buf) {
				tracing::warn!("Failed to read file {:?}: {:?}", path, e);
			}
			Ok(buf)
		}),
		None,
		format!("Failed to open file {:?}", path)
	);

	// convert the content into `Vec<u8>`
	let content_as_vec_u8 = parse_string_to_vec_u8(&content_from_file)?;

	// convert the content to TransactionEnvelope
	let env = unwrap_or_return!(
		TransactionEnvelope::from_xdr(content_as_vec_u8),
		None,
		format!("cannot decode content in file {:?}", path)
	);

	// if a sequence number is found, then this transaction envelope is acceptable;
	// otherwise mark as unacceptable.
	env.sequence_number().map(|seq| (env, seq))
}

#[cfg(test)]
mod test {
	use crate::cache::{parse_string_to_vec_u8, read_tx_envelope_from_path, TxEnvelopeStorage};
	use std::fs::read_dir;
	use substrate_stellar_sdk::{
		types::{Preconditions, SequenceNumber},
		PublicKey, Transaction, TransactionEnvelope,
	};

	const PUB_KEY: &str = "GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA";
	fn storage() -> TxEnvelopeStorage {
		TxEnvelopeStorage::new("resources/test_1".to_string(), PUB_KEY, false)
	}

	pub fn dummy_tx(sequence: SequenceNumber) -> TransactionEnvelope {
		let public_key = PublicKey::from_encoding(PUB_KEY).expect("should return a public key");

		// let's create a transaction
		let tx = Transaction::new(public_key, sequence, None, Preconditions::PrecondNone, None)
			.expect("should be able to create a tx");

		tx.into_transaction_envelope()
	}

	#[test]
	fn parse_string_to_vec_u8_success() {
		let actual_data =
			"[0,255,230,120,89,67,34,1,123,48,191,11,105,50,162,246,222,201,5]".to_string();
		let expected_result =
			vec![0, 255, 230, 120, 89, 67, 34, 1, 123, 48, 191, 11, 105, 50, 162, 246, 222, 201, 5];
		assert_eq!(parse_string_to_vec_u8(&actual_data), Some(expected_result));

		let actual_data = "[ 5 , 202 , 33, 91,03, 146 ]".to_string();
		let expected_result = vec![5, 202, 33, 91, 3, 146];
		assert_eq!(parse_string_to_vec_u8(&actual_data), Some(expected_result));
	}

	#[test]
	fn parse_string_to_vec_u8_failed() {
		let actual_data = "[256,1,80]".to_string();
		assert!(parse_string_to_vec_u8(&actual_data).is_none());

		let actual_data = "[13,111,87,-1,90]".to_string();
		assert!(parse_string_to_vec_u8(&actual_data).is_none());

		let actual_data = "[12,0,35,117,77,138,8,8080]".to_string();
		assert!(parse_string_to_vec_u8(&actual_data).is_none());

		let actual_data = "[]".to_string();
		assert!(parse_string_to_vec_u8(&actual_data).is_none());
	}

	#[test]
	fn test_read_tx_envelope_from_path() {
		let path = storage().full_path();

		let test_success = |expected_seq: SequenceNumber| {
			let file_path = format!("{path}/{expected_seq}");
			match read_tx_envelope_from_path(file_path) {
				None => assert!(false, "file {} should exist.", expected_seq),
				Some((_, actual_seq)) => assert_eq!(actual_seq, expected_seq),
			}
		};

		let seq: SequenceNumber = 17373142712629;
		test_success(seq);

		let seq: SequenceNumber = 17373142712631;
		test_success(seq);

		// file 406 Not Acceptable
		let seq: SequenceNumber = 406;
		let file_path = format!("{path}/{seq}");
		assert!(read_tx_envelope_from_path(file_path).is_none());
	}

	#[test]
	fn test_get_tx_envelopes_from_local() {
		let storage = storage();
		// testing getting 1 transaction
		let res = storage.get_tx_envelope(17373142712632);
		assert!(res.is_some());

		// file does not exist
		let res = storage.get_tx_envelope(12);
		assert!(res.is_none());

		// get all transactions
		{
			let path = storage.full_path();
			let directory = read_dir(&path).expect("should be able to read directory");

			// two of these files are invalid.
			let num_of_files =
				directory.into_iter().filter_map(|entry| entry.ok()).collect::<Vec<_>>();

			let res = storage.get_tx_envelopes();
			assert!(!res.is_empty());
			// it's <= since other tests might be creating files in parallel.
			assert!(res.len() <= (num_of_files.len() - 2));
		}

		// Create an empty storage and check that no transactions are found.
		let storage = TxEnvelopeStorage::new("test".to_string(), "test", true);
		assert!(storage.get_tx_envelopes().is_empty());
		assert!(storage.remove_all())
	}

	#[test]
	fn test_save_to_local_and_remove_transaction() {
		let sequence = 10;
		let tx = dummy_tx(sequence);

		let storage = storage();
		assert!(storage.save_to_local(sequence, tx.clone()));

		let actual_tx = storage.get_tx_envelope(sequence).expect("a tx should be found");
		assert_eq!(actual_tx, tx);

		assert!(storage.remove_transaction(sequence));
		// removing a tx again will return false.
		assert!(!storage.remove_transaction(sequence));

		// file already exists
		let sequence: SequenceNumber = 17373142712630;
		let tx = dummy_tx(sequence);
		assert!(!storage.save_to_local(sequence, tx));
	}
}
