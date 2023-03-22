use std::{
	fs::{create_dir_all, read_dir, remove_dir_all, remove_file, File, OpenOptions},
	io::{Read, Write},
	path::Path,
};
use substrate_stellar_sdk::{types::SequenceNumber, Transaction, XdrCodec};

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

/// Contains the path where the transaction will be saved.
#[derive(Debug, Clone)]
pub struct TransactionStorage {
	path: String,
	inner_path: String,
}

impl TransactionStorage {
	/// returns the full directory path
	fn full_path(&self) -> String {
		format!("{}/{}", self.path, self.inner_path)
	}

	/// initializes the `TransactionStorage` structure
	fn create(path: String, public_key: &str, is_public_network: bool) -> Self {
		let inner_path = format!("{public_key}_{is_public_network}");

		let full_path = format!("{path}/{public_key}_{is_public_network}");

		if let Err(e) = create_dir_all(&full_path) {
			tracing::warn!("Failed to create directory of {full_path}: {:?}", e);
		}

		TransactionStorage { path, inner_path }
	}
}

impl TransactionStorage {
	#[cfg(not(feature = "testing-utils"))]
	pub fn new(path: String, public_key: &str, is_public_network: bool) -> Self {
		Self::create(path, public_key, is_public_network)
	}

	#[doc(hidden)]
	#[cfg(feature = "testing-utils")]
	/// For integration tests, `TempDir` will be used instead.
	pub fn new(path: String, public_key: &str, is_public_network: bool) -> Self {
		let tmp = tempdir::TempDir::new("cargo_test").expect("failed to create tempdir");
		let path = tmp.path().to_str().map(|x| x.to_string()).unwrap_or(path);
		Self::create(path, public_key, is_public_network)
	}

	/// Returns true if transaction was saved to a local path.
	/// The filename will be the transaction's sequence number.
	pub fn save_to_local(&self, transaction: Transaction) -> bool {
		let full_file_path = format!("{}/{}", self.full_path(), transaction.seq_num);
		let path = Path::new(&full_file_path);

		if path.exists() {
			return false
		}

		let mut file = unwrap_or_return!(
			OpenOptions::new().write(true).create(true).open(path),
			false,
			format!("Failed to create file for transaction with sequence {}", transaction.seq_num)
		);

		unwrap_or_return!(
			// writing the transaction to file in Vec<u8> format
			write!(file, "{:?}", transaction.to_xdr()),
			false,
			format!("Failed to write transaction with sequence {}", transaction.seq_num)
		);

		true
	}

	/// Returns true if the transaction file was successfully removed.
	pub fn remove_transaction(&self, sequence: SequenceNumber) -> bool {
		let full_file_path = format!("{}/{sequence}", self.full_path());
		remove_file(full_file_path).is_ok()
	}

	#[doc(hidden)]
	#[cfg(any(test, feature = "testing-utils"))]
	/// Removes all transactions
	pub fn remove_all(&self) -> bool {
		remove_dir_all(self.full_path()).is_ok()
	}

	/// Returns a transaction if a file was found, given the sequence number
	pub fn get_transaction(&self, sequence: SequenceNumber) -> Option<Transaction> {
		let full_file_path = format!("{}/{sequence}", self.full_path());
		let path = Path::new(&full_file_path);

		if !path.exists() {
			return None
		}
		read_transaction_from_file(path)
	}

	/// Returns all transactions found in local path
	pub fn get_transactions(&self) -> Vec<Transaction> {
		let path = self.full_path();
		let directory =
			unwrap_or_return!(read_dir(&path), vec![], format!("Failed to read directory {path}"));

		directory
			.into_iter()
			.filter_map(|entry| {
				let path = entry.ok()?.path();
				read_transaction_from_file(path)
			})
			.collect()
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

/// a helper function to parse a content of a file into `Transaction`.
fn read_transaction_from_file<P: AsRef<Path> + std::fmt::Debug + Clone>(
	path: P,
) -> Option<Transaction> {
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
	// convert the content to Transaction
	Transaction::from_xdr(content_as_vec_u8).ok()
}

#[cfg(test)]
mod test {

	impl TransactionStorage {}
	use crate::cache::{parse_string_to_vec_u8, read_transaction_from_file, TransactionStorage};
	use std::fs;
	use substrate_stellar_sdk::{
		types::{Preconditions, SequenceNumber},
		PublicKey, Transaction,
	};

	const PUB_KEY: &str = "GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA";
	fn storage() -> TransactionStorage {
		TransactionStorage::new("resources".to_string(), PUB_KEY, false)
	}

	fn dummy_tx(sequence: SequenceNumber) -> Transaction {
		let public_key = PublicKey::from_encoding(PUB_KEY).expect("should return a public key");

		// let's create a transaction
		Transaction::new(public_key, sequence, None, Preconditions::PrecondNone, None)
			.expect("should be able to create a tx")
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
	fn test_read_transaction_from_file() {
		let path = storage().full_path();

		let test_success = |seq: SequenceNumber| {
			let file_path = format!("{path}/{seq}");
			match read_transaction_from_file(file_path) {
				None => assert!(false, "file {} should exist.", seq),
				Some(tx) => assert_eq!(tx.seq_num, seq),
			}
		};

		let seq: SequenceNumber = 17373142712446;
		test_success(seq);

		let seq: SequenceNumber = 17373142712447;
		test_success(seq);

		// file 406 Not Acceptable
		let seq: SequenceNumber = 406;
		let file_path = format!("{path}/{seq}");
		assert!(read_transaction_from_file(file_path).is_none());
	}

	#[test]
	fn test_get_transactions_from_local() {
		let storage = storage();
		// get only 1 transaction
		let res = storage.get_transaction(17373142712445);
		assert!(res.is_some());

		// get all transactions
		let res = storage.get_transactions();
		assert!(!res.is_empty());

		// file does not exist
		let res = storage.get_transaction(12);
		assert!(res.is_none());

		// let's create an empty storage and no transactions are found.
		let storage = TransactionStorage::new("test".to_string(), "test", true);
		assert!(storage.get_transactions().is_empty());
		assert!(storage.remove_all())
	}

	#[test]
	fn test_save_to_local_and_remove_transaction() {
		let sequence = 10;
		let tx = dummy_tx(sequence);

		let storage = storage();
		assert!(storage.save_to_local(tx.clone()));

		let actual_tx = storage.get_transaction(sequence).expect("a tx should be found");
		assert_eq!(actual_tx, tx);

		assert!(storage.remove_transaction(sequence));
		// removing a tx again will return false.
		assert!(!storage.remove_transaction(sequence));

		// file already exists
		let seq: SequenceNumber = 17373142712448;
		let tx = dummy_tx(seq);
		assert!(!storage.save_to_local(tx));
	}
}
