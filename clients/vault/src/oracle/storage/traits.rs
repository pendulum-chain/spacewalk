use sp_core::hexdisplay::AsBytesRef;
use stellar_relay::sdk::{
	compound_types::XdrArchive,
	types::{ScpEnvelope, ScpHistoryEntry, TransactionHistoryEntry, TransactionSet},
	XdrCodec,
};

use flate2::bufread::GzDecoder;

use crate::oracle::{constants::ARCHIVE_NODE_LEDGER_BATCH, Error, Filename, SerializedData, Slot};
use std::{
	fs,
	fs::File,
	io::{Read, Write},
	path::PathBuf,
	str::Split,
};

pub trait FileHandlerExt<T: Default>: FileHandler<T> {
	fn create_filename_and_data(data: &T) -> Result<(Filename, SerializedData), Error>;

	fn write_to_file(data: &T) -> Result<Filename, Error> {
		let (filename, data) = Self::create_filename_and_data(data)?;

		let path = Self::get_path(&filename);
		let mut file = File::create(path)?;

		file.write_all(&data)?;

		Ok(filename)
	}
}

pub trait FileHandler<T: Default> {
	// path to where the file should be saved
	const PATH: &'static str;

	fn deserialize_bytes(bytes: Vec<u8>) -> Result<T, Error>;

	fn check_slot_in_splitted_filename(slot_param: Slot, splits: &mut Split<&str>) -> bool;

	fn get_path(filename: &str) -> PathBuf {
		let mut path = PathBuf::new();
		path.push(Self::PATH);
		path.push(filename);
		path
	}

	fn read_file(filename: &str) -> Result<T, Error> {
		let path = Self::get_path(filename);
		let mut file = File::open(path)?;

		let mut bytes: Vec<u8> = vec![];
		let read_size = file.read_to_end(&mut bytes)?;

		if read_size > 0 {
			return Self::deserialize_bytes(bytes)
		}

		Ok(T::default())
	}

	fn find_file_by_slot(slot_param: Slot) -> Result<String, Error> {
		let paths = fs::read_dir(Self::PATH)?;

		for path in paths {
			let filename = path?.file_name().into_string().unwrap();
			let mut splits = filename.split("_");

			if Self::check_slot_in_splitted_filename(slot_param, &mut splits) {
				return Ok(filename)
			}
		}

		Err(Error::Other(format!("Cannot find file for slot {}", slot_param)))
	}

	fn get_map_from_archives(slot: Slot) -> Result<T, Error> {
		let filename = Self::find_file_by_slot(slot)?;

		Self::read_file(&filename)
	}
}

pub trait ArchiveStorage {
	type T: XdrCodec;
	const STELLAR_HISTORY_BASE_URL: &'static str;
	const prefix_url: &'static str;
	const prefix_filename: &'static str = "";

	fn try_gz_decode_archive_file(path: &str) -> Result<Vec<u8>, Error> {
		let bytes = Self::read_file_xdr(path)?;
		let mut gz = GzDecoder::new(&bytes[..]);
		let mut bytes: Vec<u8> = vec![];
		gz.read_to_end(&mut bytes)?;
		Ok(bytes)
	}

	fn get_url_and_file_name(slot_index: i32) -> (String, String) {
		let slot_index = Self::find_last_slot_index_in_batch(slot_index);
		let hex_string = format!("0{:x}", slot_index);
		let file_name = format!("{hex_string}.xdr");
		let base_url = Self::STELLAR_HISTORY_BASE_URL;
		let url = format!(
			"{base_url}{}/{}/{}/{}-{file_name}.gz",
			&hex_string[..2],
			&hex_string[2..4],
			&hex_string[4..6],
			Self::prefix_url
		);
		(url, format!("{}{file_name}", Self::prefix_filename))
	}

	fn find_last_slot_index_in_batch(slot_index: i32) -> i32 {
		let rest = (slot_index + 1) % ARCHIVE_NODE_LEDGER_BATCH;
		if rest == 0 {
			return slot_index
		}
		return slot_index + ARCHIVE_NODE_LEDGER_BATCH - rest
	}

	fn read_file_xdr(filename: &str) -> Result<Vec<u8>, Error> {
		let mut file = File::open(filename)?;
		let mut bytes: Vec<u8> = vec![];
		file.read_to_end(&mut bytes)?;
		Ok(bytes)
	}

	fn decode_xdr(xdr_data: Vec<u8>) -> XdrArchive<Self::T> {
		XdrArchive::<Self::T>::from_xdr(xdr_data).unwrap()
	}
}

pub(crate) async fn download_file_and_save(url: &str, file_name: &str) -> Result<(), Error> {
	let response = reqwest::get(url).await.unwrap();
	let content = response.bytes().await.unwrap();

	let mut file = match File::create(&file_name) {
		Err(why) => panic!("couldn't create {}: {}", &file_name, why),
		Ok(file) => file,
	};
	file.write_all(content.as_bytes_ref())?;
	Ok(())
}
