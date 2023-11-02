use crate::{
	error::{CacheErrorKind, Error},
	types::PagingToken,
};
use primitives::{
	stellar::{types::SequenceNumber, TransactionEnvelope, XdrCodec},
	TransactionEnvelopeExt,
};
use std::{
	fs::{create_dir_all, read_dir, remove_file, File, OpenOptions},
	io::{Read, Write},
	path::Path,
};

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
}

/// Contains the path where the transaction envelope and the cursor/paging token will be saved.
#[derive(Debug, Clone)]
pub struct WalletStateStorage {
	path: String,
	inner_path: String,
}

impl WalletStateStorage {
	fn root_path(&self) -> String {
		format!("{}/{}", self.path, self.inner_path)
	}

	pub fn new(path: String, public_key: &str, is_public_network: bool) -> Self {
		let inner_path = format!("{public_key}_{is_public_network}");
		let cache = WalletStateStorage { path, inner_path };

		let txs_full_path = cache.txs_inner_dir();
		if let Err(e) = create_dir_all(&txs_full_path) {
			tracing::warn!("Failed to create directory of {txs_full_path}: {:?}", e);
		} else {
			tracing::info!("Caching stellar transactions at {txs_full_path}");
		}

		cache
	}
}

// methods for saving/retrieving the cursor / paging_token
impl WalletStateStorage {
	const CURSOR_FILENAME: &'static str = "cursor";

	fn cursor_path(&self) -> String {
		format!("{}/{}", self.root_path(), Self::CURSOR_FILENAME)
	}

	/// returns the latest cursor  of the given wallet
	pub fn get_last_cursor(&self) -> PagingToken {
		let path = self.cursor_path();
		let path = Path::new(&path);
		if !path.exists() {
			return 0
		}

		let Ok(content_from_file) = read_content_from_path(path) else {
			return 0;
		};

		content_from_file.parse::<u128>().unwrap_or(0)
	}

	/// saves the paging token as a cursor
	pub fn save_cursor(&self, paging_token: PagingToken) -> Result<(), Error> {
		let path = self.cursor_path();

		let mut file = OpenOptions::new().write(true).create(true).open(&path).map_err(|e| {
			tracing::error!("Failed to create file {path:?}: {e:?}");

			Error::cache_error_with_path(CacheErrorKind::FileCreationFailed, path.clone())
		})?;

		write!(file, "{}", paging_token).map_err(|e| {
			tracing::error!("Failed to write file: {paging_token:?}: {e:?}");

			Error::cache_error_with_path(CacheErrorKind::WriteToFileFailed, path)
		})
	}

	#[allow(dead_code)]
	#[doc(hidden)]
	#[cfg(any(test, feature = "testing-utils"))]
	/// Necessary for testing.
	/// User should not be able to do this in production.
	pub fn remove_cursor(&self) -> Result<(), Error> {
		let full_file_path = self.cursor_path();
		remove_file(&full_file_path).map_err(|e| {
			tracing::error!("Failed to delete file: {:?}", e);
			Error::cache_error_with_path(CacheErrorKind::DeleteFileFailed, full_file_path)
		})
	}
}

// methods for tx envelope
impl WalletStateStorage {
	const TXS_INNER_DIR: &'static str = "txs";

	fn txs_inner_dir(&self) -> String {
		format!("{}/{}", self.root_path(), Self::TXS_INNER_DIR)
	}

	/// Saves the transaction envelope to a local folder.
	/// The filename will be the transaction's sequence number.
	pub fn save_tx_envelope(&self, tx_envelope: TransactionEnvelope) -> Result<(), Error> {
		let sequence = tx_envelope.sequence_number().ok_or(Error::cache_error_with_env(
			CacheErrorKind::UnknownSequenceNumber,
			tx_envelope.clone(),
		))?;

		let full_file_path = format!("{}/{sequence}", self.txs_inner_dir());

		let path = Path::new(&full_file_path);
		if path.exists() {
			return Err(Error::cache_error_with_seq(
				CacheErrorKind::SequenceNumberAlreadyUsed,
				sequence,
			))
		}

		let mut file = OpenOptions::new().write(true).create(true).open(path).map_err(|e| {
			tracing::error!("Failed to create file {path:?}: {e:?}");

			Error::cache_error_with_env(CacheErrorKind::FileCreationFailed, tx_envelope.clone())
		})?;

		write!(file, "{:?}", tx_envelope.to_xdr()).map_err(|e| {
			tracing::error!("Failed to write file: {tx_envelope:?}: {e:?}");

			Error::cache_error_with_env(CacheErrorKind::WriteToFileFailed, tx_envelope)
		})
	}

	/// Removes a transaction from the local folder
	pub fn remove_tx_envelope(&self, sequence: SequenceNumber) {
		let full_file_path = format!("{}/{sequence}", self.txs_inner_dir());
		match remove_file(&full_file_path) {
			Ok(_) => tracing::debug!("remove_tx_envelope(): Deleted file with sequence {sequence}"),
			Err(e) => tracing::error!(
				"remove_tx_envelope(): Failed to delete file with sequence {sequence}: {e:?}"
			)
		}

	}

	#[allow(dead_code)]
	#[doc(hidden)]
	#[cfg(any(test, feature = "testing-utils"))]
	/// Removes the directory itself.
	/// User should not be able to do this in production.
	pub fn remove_dir(&self) {
		if Path::new(&self.path).is_dir() {
			let path = &self.path;
			if let Err(e) = std::fs::remove_dir_all(path) {
				tracing::warn!("failed to delete {path}: {e:?}. Please delete manually");
			}
		}
	}

	#[doc(hidden)]
	#[cfg(any(test, feature = "testing-utils"))]
	#[allow(dead_code)]
	/// Removes all transactions in the directory.
	/// User should not be able to do this in production.
	pub fn remove_all_tx_envelopes(&self) {
		let full_path = self.txs_inner_dir();
		let Ok(directory) = read_dir(&full_path) else {
			// create a new one
			if let Err(e) = create_dir_all(&full_path) {
				tracing::warn!("Failed to create directory of {full_path}: {:?}", e);
			}
			return;
		};

		for entry in directory.flatten() {
			if let Err(e) = remove_file(entry.path()) {
				tracing::warn!("Failed to remove {full_path}: {:?}", e);
			}
		}
	}

	#[allow(dead_code)]
	#[cfg(any(test, feature = "testing-utils"))]
	/// Returns a transaction if a file was found, given the sequence number
	pub fn get_tx_envelope(&self, sequence: SequenceNumber) -> Result<TransactionEnvelope, Error> {
		let full_file_path = format!("{}/{sequence}", self.txs_inner_dir());
		let path = Path::new(&full_file_path);

		if !path.exists() {
			return Err(Error::cache_error_with_seq(CacheErrorKind::FileDoesNotExist, sequence))
		}

		extract_tx_envelope_from_path(path).map(|(tx, _)| tx)
	}

	/// Returns a list of transactions found in the local path, else a list of errors.
	pub fn get_tx_envelopes(&self) -> Result<(Vec<TransactionEnvelope>, Vec<Error>), Vec<Error>> {
		let path = self.txs_inner_dir();
		let directory = read_dir(&path).map_err(|e| {
			tracing::error!("Could not read path: {path:?}: {e:?}");
			vec![Error::cache_error_with_path(CacheErrorKind::ReadFileFailed, format!("{path:?}"))]
		})?;

		let mut errors = vec![];
		let mut tx_envelopes: Vec<(TransactionEnvelope, SequenceNumber)> = directory
			.into_iter()
			// filter only the files that are successfully decoded and extracted.
			.filter_map(|entry| {
				let path = entry.ok()?.path();

				match extract_tx_envelope_from_path(&path) {
					Ok(env) => Some(env),
					Err(e) => {
						// removes invalid files.
						// For unit-testing, the invalid files are important to test failing cases.
						#[cfg(not(test))]
						if remove_file(&path).is_err() {
							tracing::warn!("failed to remove unreadable file: {:?}", path);
						}

						tracing::error!("{:?}", e);

						// collect also the failed ones
						errors.push(e);

						None
					},
				}
			})
			.collect();

		// return an error if all the files have errors.
		if tx_envelopes.is_empty() && !errors.is_empty() {
			return Err(errors)
		}

		// sort in ascending order, based on the sequence number.
		tx_envelopes.sort_by(|(_, first), (_, next)| first.cmp(&next));

		Ok((tx_envelopes.into_iter().map(|(env, _)| env).collect(), errors))
	}
}

/// a helper function to convert a String content into `Vec<u8>`
fn parse_xdr_string_to_vec_u8(value: &str) -> Option<Vec<u8>> {
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
/// Otherwise returns an Error
fn extract_tx_envelope_from_path<P: AsRef<Path> + std::fmt::Debug + Clone>(
	path: P,
) -> Result<(TransactionEnvelope, SequenceNumber), Error> {
	let content_from_file = read_content_from_path(&path)?;

	// convert the content into `Vec<u8>`
	let Some(content_as_vec_u8) = parse_xdr_string_to_vec_u8(&content_from_file) else {
		return Err(Error::cache_error_with_path(CacheErrorKind::InvalidFile, format!("{path:?}")));
	};

	// convert the content to TransactionEnvelope
	let env = TransactionEnvelope::from_xdr(content_as_vec_u8).map_err(|e| {
		tracing::error!("Cannot decode file: {e:?}");

		Error::cache_error_with_path(CacheErrorKind::DecodeFileFailed, format!("{path:?}"))
	})?;

	// if a sequence number is found, then this transaction envelope is acceptable;
	// otherwise mark as unacceptable.
	env.sequence_number()
		.map(|seq| (env.clone(), seq))
		.ok_or(Error::cache_error_with_env(CacheErrorKind::UnknownSequenceNumber, env))
}

#[doc(hidden)]
/// A helper function to read the content of the file as string, given the path.
fn read_content_from_path<P: AsRef<Path> + std::fmt::Debug + Clone>(
	path: P,
) -> Result<String, Error> {
	OpenOptions::new()
		.read(true)
		.open(&path)
		.map(|mut file: File| {
			// if the file exists, read the contents as a string.
			let mut buf = String::new();

			if let Err(e) = file.read_to_string(&mut buf) {
				tracing::error!("Failed to read file {path:?}: {e:?}");
			}

			buf
		})
		.map_err(|e| {
			tracing::error!("Failed to read file {:?}", e);
			Error::cache_error_with_path(CacheErrorKind::ReadFileFailed, format!("{path:?}"))
		})
}

#[cfg(test)]
mod test {
	use crate::{
		cache::{
			extract_tx_envelope_from_path, parse_xdr_string_to_vec_u8, Error, WalletStateStorage,
		},
		error::CacheErrorKind,
		mock::public_key_from_encoding,
	};
	use primitives::{
		stellar::{
			types::{Preconditions, SequenceNumber},
			Transaction, TransactionEnvelope,
		},
		TransactionEnvelopeExt,
	};
	use std::{fmt::Debug, fs::read_dir};

	const PUB_KEY: &str = "GCENYNAX2UCY5RFUKA7AYEXKDIFITPRAB7UYSISCHVBTIAKPU2YO57OA";
	fn storage() -> WalletStateStorage {
		WalletStateStorage::new("resources/examples".to_string(), PUB_KEY, false)
	}

	pub fn dummy_tx(sequence: SequenceNumber) -> TransactionEnvelope {
		let public_key = public_key_from_encoding(PUB_KEY);

		// let's create a transaction
		let tx = Transaction::new(public_key, sequence, None, Preconditions::PrecondNone, None)
			.expect("should be able to create a tx");

		tx.into_transaction_envelope()
	}

	fn assert_error<T: Debug>(result: Result<T, Error>, expected_errorkind: CacheErrorKind) {
		assert!(result.is_err());
		match result.unwrap_err() {
			Error::CacheError(actual_error) => assert_eq!(actual_error.kind, expected_errorkind),
			_ => assert!(false),
		};
	}

	#[test]
	fn parse_xdr_string_to_vec_u8_success() {
		let actual_data =
			"[0,255,230,120,89,67,34,1,123,48,191,11,105,50,162,246,222,201,5]".to_string();
		let expected_result =
			vec![0, 255, 230, 120, 89, 67, 34, 1, 123, 48, 191, 11, 105, 50, 162, 246, 222, 201, 5];
		assert_eq!(parse_xdr_string_to_vec_u8(&actual_data), Some(expected_result));

		let actual_data = "[ 5 , 202 , 33, 91,03, 146 ]".to_string();
		let expected_result = vec![5, 202, 33, 91, 3, 146];
		assert_eq!(parse_xdr_string_to_vec_u8(&actual_data), Some(expected_result));
	}

	#[test]
	fn parse_xdr_string_to_vec_u8_failed() {
		let actual_data = "[256,1,80]".to_string();
		assert!(parse_xdr_string_to_vec_u8(&actual_data).is_none());

		let actual_data = "[13,111,87,-1,90]".to_string();
		assert!(parse_xdr_string_to_vec_u8(&actual_data).is_none());

		let actual_data = "[12,0,35,117,77,138,8,8080]".to_string();
		assert!(parse_xdr_string_to_vec_u8(&actual_data).is_none());

		let actual_data = "[]".to_string();
		assert!(parse_xdr_string_to_vec_u8(&actual_data).is_none());
	}

	#[test]
	fn test_extract_tx_envelope_from_path() {
		let path = storage().txs_inner_dir();

		let test_success = |expected_seq: SequenceNumber| {
			let file_path = format!("{path}/{expected_seq}");

			let extract_result = extract_tx_envelope_from_path(file_path);
			assert!(extract_result.is_ok());

			let (_, actual_seq) = extract_result.expect("should return Ok");
			assert_eq!(actual_seq, expected_seq);
		};

		let seq: SequenceNumber = 17373142712629;
		test_success(seq);

		let seq: SequenceNumber = 17373142712631;
		test_success(seq);

		// file 406 Not Acceptable
		let seq: SequenceNumber = 406;
		let file_path = format!("{path}/{seq}");
		assert_error(extract_tx_envelope_from_path(&file_path), CacheErrorKind::InvalidFile);
	}

	#[test]
	fn test_get_tx_envelopes() {
		let storage = storage();
		let expected_seq = 17373142712632;
		// testing getting 1 transaction
		let res = storage.get_tx_envelope(expected_seq);
		assert!(res.is_ok());
		let actual_seq = res.expect("should return an envelope").sequence_number();
		assert_eq!(actual_seq, Some(expected_seq));

		// file does not exist
		let seq = 12;
		assert_error(storage.get_tx_envelope(seq), CacheErrorKind::FileDoesNotExist);

		// get all transactions
		let path = storage.txs_inner_dir();
		let directory = read_dir(&path).expect("should be able to read directory");

		// two of these files are invalid.
		let num_of_files = directory.into_iter().filter_map(|entry| entry.ok()).collect::<Vec<_>>();

		let (actual_envelopes, actual_errors) =
			storage.get_tx_envelopes().expect("should return ok");

		// since 2 files are invalid, the errors should have 2.
		assert_eq!(actual_errors.len(), 2);

		// it's <= since other tests might be creating files in parallel.
		assert!(actual_envelopes.len() <= (num_of_files.len() - actual_errors.len()));

		// Create an empty storage and check that no transactions are found.
		let storage = WalletStateStorage::new("test".to_string(), "test", true);
		let (actual_envelopes, actual_errors) =
			storage.get_tx_envelopes().expect("should return ok");
		assert!(actual_envelopes.is_empty());
		assert!(actual_errors.is_empty());
		storage.remove_dir();
	}

	#[test]
	fn test_save_tx_envelope_and_remove() {
		let sequence = 10;
		let expected_tx = dummy_tx(sequence);

		// let's create a new storage
		let new_storage = WalletStateStorage::new(
			"resources/test_save_tx_envelope_and_remove".to_string(),
			PUB_KEY,
			false,
		);

		// clear it first
		new_storage.remove_all_tx_envelopes();

		assert!(new_storage.save_tx_envelope(expected_tx.clone()).is_ok());

		let actual_tx = new_storage.get_tx_envelope(sequence).expect("a tx should be found");
		assert_eq!(actual_tx, expected_tx);

		new_storage.remove_tx_envelope(sequence);

		// let's remove the entire directory
		new_storage.remove_dir();

		// file already exists
		let seq = 17373142712630;
		let tx = dummy_tx(seq);
		assert_error(storage().save_tx_envelope(tx), CacheErrorKind::SequenceNumberAlreadyUsed);
	}

	#[test]
	fn test_cursors() {
		// empty cursor file
		let storage = storage();
		assert_eq!(storage.get_last_cursor(), 0);

		// save a cursor
		let expected_cursor = 10;
		storage.save_cursor(expected_cursor).expect("should save alright");

		// cursor should return a non-zero value
		assert_eq!(storage.get_last_cursor(), expected_cursor);
		assert!(storage.remove_cursor().is_ok());
	}
}
