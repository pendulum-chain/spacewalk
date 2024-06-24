use std::{
	fs,
	fs::File,
	io::{Read, Write},
	path::PathBuf,
	str::Split,
};

use flate2::bufread::GzDecoder;
use sp_core::hexdisplay::AsBytesRef;

use stellar_relay_lib::sdk::{compound_types::XdrArchive, XdrCodec};

use crate::oracle::{constants::ARCHIVE_NODE_LEDGER_BATCH, Error, Filename, SerializedData};
use wallet::Slot;

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

	fn check_slot_in_splitted_filename(slot_param: Slot, splits: &mut Split<char>) -> bool;

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
			let filename = path?.file_name().into_string().map_err(|e| {
				Error::Other(format!("Failed to convert filename to string: {e:?}"))
			})?;
			let mut splits: Split<char> = filename.split('_');

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

#[async_trait::async_trait]
pub trait ArchiveStorage {
	type T: XdrCodec;
	const PREFIX_URL: &'static str;
	const PREFIX_FILENAME: &'static str = "";

	fn stellar_history_base_url(&self) -> String;

	async fn get_archive(
		&self,
		slot_index: Slot,
	) -> Result<XdrArchive<<Self as ArchiveStorage>::T>, Error> {
		let (url, file_name) = self.get_url_and_file_name(slot_index);
		//try to find xdr.gz file and decode. if error then download archive from horizon archive
		// node and save
		let mut result = Self::try_gz_decode_archive_file(&file_name);

		if result.is_err() {
			download_file_and_save(&url, &file_name).await?;
			result = Self::try_gz_decode_archive_file(&file_name);
		}
		Ok(Self::decode_xdr(result?)?)
	}

	fn try_gz_decode_archive_file(path: &str) -> Result<Vec<u8>, Error> {
		let bytes = Self::read_file_xdr(path)?;
		let mut gz = GzDecoder::new(&bytes[..]);
		let mut bytes: Vec<u8> = vec![];
		gz.read_to_end(&mut bytes)?;
		Ok(bytes)
	}

	fn get_url_and_file_name(&self, slot_index: Slot) -> (String, String) {
		let slot_index = self.find_last_slot_index_in_batch(slot_index);
		let hex_string = format!("{:08x}", slot_index);

		let file_name = format!("{hex_string}.xdr");
		let base_url = self.stellar_history_base_url();
		let url = format!(
			"{base_url}/{}/{}/{}/{}/{}-{file_name}.gz",
			Self::PREFIX_URL.trim_end_matches('/'),
			&hex_string[..2],
			&hex_string[2..4],
			&hex_string[4..6],
			Self::PREFIX_URL
		);
		(url, format!("{}{file_name}", Self::PREFIX_FILENAME))
	}

	fn find_last_slot_index_in_batch(&self, slot_index: Slot) -> Slot {
		let rest = (slot_index + 1) % ARCHIVE_NODE_LEDGER_BATCH;
		if rest == 0 {
			return slot_index
		}
		slot_index + ARCHIVE_NODE_LEDGER_BATCH - rest
	}

	fn remove_file(&self, target_slot: Slot) {
		let (_, file) = self.get_url_and_file_name(target_slot);
		if let Err(e) = fs::remove_file(&file) {
			tracing::warn!(
				"remove_file(): failed to remove file {file} for slot {target_slot}: {e:?}"
			);
		}
	}

	fn read_file_xdr(filename: &str) -> Result<Vec<u8>, Error> {
		let mut file = File::open(filename)?;
		let mut bytes: Vec<u8> = vec![];
		file.read_to_end(&mut bytes)?;
		Ok(bytes)
	}

	fn decode_xdr(xdr_data: Vec<u8>) -> Result<XdrArchive<Self::T>, Error> {
		XdrArchive::<Self::T>::from_xdr(xdr_data)
			.map_err(|e| Error::Other(format!("Decode Error: {e:?}")))
	}
}

pub(crate) async fn download_file_and_save(url: &str, file_name: &str) -> Result<(), Error> {
	let response = reqwest::get(url).await.map_err(|e| Error::ArchiveError(e.to_string()))?;
	if response.status().is_server_error() | response.status().is_client_error() {
		return Err(Error::ArchiveResponseError(format!("{response:?}")));
	}

	let content = response.bytes().await.map_err(|e| Error::ArchiveError(e.to_string()))?;
	let mut file = File::create(&file_name).map_err(|e| Error::ArchiveError(e.to_string()))?;
	file.write_all(content.as_bytes_ref())
		.map_err(|e| Error::ArchiveError(e.to_string()))?;
	Ok(())
}
