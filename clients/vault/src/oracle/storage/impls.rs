use sp_core::hexdisplay::AsBytesRef;
use std::{
	fs::{create_dir_all, File},
	io::{Read, Write},
	str::Split,
};
use stellar_relay::sdk::{
	compound_types::{UnlimitedVarArray, XdrArchive},
	types::{ScpEnvelope, ScpHistoryEntry, TransactionSet, TransactionHistoryEntry},
};

use crate::oracle::{
	storage::traits::*, EnvelopesFileHandler, EnvelopesMap, Error, Filename, SerializedData, Slot,
	SlotEncodedMap, TxHashMap, TxHashesFileHandler, TxSetMap, TxSetsFileHandler,
};

use stellar_relay::sdk::XdrCodec;

use super::{ScpArchiveStorage, TransactionsArchiveStorage};

impl FileHandler<EnvelopesMap> for EnvelopesFileHandler {
	#[cfg(test)]
	const PATH: &'static str = "./resources/test/scp_envelopes";

	#[cfg(not(test))]
	const PATH: &'static str = "./scp_envelopes";

	fn deserialize_bytes(bytes: Vec<u8>) -> Result<EnvelopesMap, Error> {
		let inside: SlotEncodedMap = bincode::deserialize(&bytes)?;

		let mut m: EnvelopesMap = EnvelopesMap::new();
		for (key, value) in inside.into_iter() {
			if let Ok(envelopes) = UnlimitedVarArray::<ScpEnvelope>::from_xdr(value) {
				m.insert(key, envelopes.get_vec().to_vec());
			}
		}

		Ok(m)
	}

	fn check_slot_in_splitted_filename(slot_param: Slot, splits: &mut Split<&str>) -> bool {
		fn parse_slot(slot_opt: Option<&str>) -> Option<Slot> {
			(slot_opt?).parse::<Slot>().ok()
		}

		if let Some(start_slot) = parse_slot(splits.next()) {
			if let Some(end_slot) = parse_slot(splits.next()) {
				return (slot_param >= start_slot) && (slot_param <= end_slot)
			}
		}

		false
	}
}

impl FileHandlerExt<EnvelopesMap> for EnvelopesFileHandler {
	fn create_filename_and_data(data: &EnvelopesMap) -> Result<(Filename, SerializedData), Error> {
		let mut filename: Filename = "".to_string();
		let mut m: SlotEncodedMap = SlotEncodedMap::new();
		let len = data.len();

		for (idx, (key, value)) in data.iter().enumerate() {
			if idx == 0 {
				filename.push_str(&format!("{}_", key));
			}

			if idx == (len - 1) {
				filename.push_str(&format!("{}", key));
			}

			let stellar_array = UnlimitedVarArray::new(value.clone())?;
			m.insert(*key, stellar_array.to_xdr());
		}

		let res = bincode::serialize(&m)?;

		Ok((filename, res))
	}
}

impl FileHandler<TxSetMap> for TxSetsFileHandler {
	#[cfg(test)]
	const PATH: &'static str = "./resources/test/tx_sets";

	#[cfg(not(test))]
	const PATH: &'static str = "./tx_sets";

	fn deserialize_bytes(bytes: Vec<u8>) -> Result<TxSetMap, Error> {
		let inside: SlotEncodedMap = bincode::deserialize(&bytes)?;

		let mut m: TxSetMap = TxSetMap::new();

		for (key, value) in inside.into_iter() {
			if let Ok(set) = TransactionSet::from_xdr(value) {
				m.insert(key, set);
			}
		}

		Ok(m)
	}

	fn check_slot_in_splitted_filename(slot_param: Slot, splits: &mut Split<&str>) -> bool {
		EnvelopesFileHandler::check_slot_in_splitted_filename(slot_param, splits)
	}
}

impl FileHandlerExt<TxSetMap> for TxSetsFileHandler {
	fn create_filename_and_data(data: &TxSetMap) -> Result<(Filename, SerializedData), Error> {
		let mut filename: Filename = "".to_string();
		let mut m: SlotEncodedMap = SlotEncodedMap::new();
		let len = data.len();

		for (idx, (key, set)) in data.iter().enumerate() {
			if idx == 0 {
				filename.push_str(&format!("{}_", key));
			}

			if idx == (len - 1) {
				filename.push_str(&format!("{}", key));
			}

			m.insert(*key, set.to_xdr());
		}

		Ok((filename, bincode::serialize(&m)?))
	}
}

impl TxHashesFileHandler {
	fn create_data(data: &TxHashMap) -> Result<SerializedData, Error> {
		bincode::serialize(data).map_err(Error::from)
	}

	pub fn write_to_file(filename: Filename, data: &TxHashMap) -> Result<(), Error> {
		let path = Self::get_path(&filename);
		let mut file = File::create(path)?;

		let data = Self::create_data(data)?;
		file.write_all(&data).map_err(Error::from)
	}
}

impl FileHandler<TxHashMap> for TxHashesFileHandler {
	#[cfg(test)]
	const PATH: &'static str = "./resources/test/tx_hashes";

	#[cfg(not(test))]
	const PATH: &'static str = "./tx_hashes";

	fn deserialize_bytes(bytes: Vec<u8>) -> Result<TxHashMap, Error> {
		bincode::deserialize(&bytes).map_err(Error::from)
	}

	fn check_slot_in_splitted_filename(slot_param: Slot, splits: &mut Split<&str>) -> bool {
		TxSetsFileHandler::check_slot_in_splitted_filename(slot_param, splits)
	}
}

impl ScpArchiveStorage {
	pub async fn get_scp_archive(slot_index: i32) -> Result<XdrArchive<ScpHistoryEntry>, Error> {
		let (url, file_name) = Self::get_url_and_file_name(slot_index);
		//try to find xdr.gz file and decode. if error then download archive from horizon archive
		// node and save
		let result = Self::try_gz_decode_archive_file(&file_name);

		if result.is_err() {
			let result = Self::download_file_and_save(&url, &file_name).await;
			if result.is_ok() {
				let data = Self::try_gz_decode_archive_file(&file_name)?;
				return Ok(Self::decode_xdr(data))
			}
		}
		let data = result.unwrap();
		Ok(Self::decode_xdr(data))
	}

	fn decode_xdr(xdr_data: Vec<u8>) -> XdrArchive<ScpHistoryEntry> {
		XdrArchive::<ScpHistoryEntry>::from_xdr(xdr_data).unwrap()
	}

	async fn download_file_and_save(url: &str, file_name: &str) -> Result<(), Error> {
		let response = reqwest::get(url).await.unwrap();
		let content = response.bytes().await.unwrap();

		let mut file = match File::create(&file_name) {
			Err(why) => panic!("couldn't create {}", why),
			Ok(file) => file,
		};
		file.write_all(content.as_bytes_ref())?;
		Ok(())
	}

	fn try_gz_decode_archive_file(path: &str) -> Result<Vec<u8>, Error> {
		use flate2::bufread::GzDecoder;
		use std::io::{self, BufReader, Read};
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
		let base_url = crate::oracle::constants::stellar_history_base_url;
		let url = format!(
			"{base_url}{}/{}/{}/scp-{file_name}.gz",
			&hex_string[..2],
			&hex_string[2..4],
			&hex_string[4..6]
		);
		(url, file_name)
	}

	fn find_last_slot_index_in_batch(slot_index: i32) -> i32 {
		let rest = (slot_index + 1) % 64;
		if rest == 0 {
			return slot_index
		}
		return slot_index + 64 - rest
	}

	fn read_file_xdr(filename: &str) -> Result<Vec<u8>, Error> {
		let mut file = File::open(filename)?;
		let mut bytes: Vec<u8> = vec![];
		file.read_to_end(&mut bytes)?;
		Ok(bytes)
	}
}

impl TransactionsArchiveStorage {
	pub async fn get_transactions_archive(slot_index: i32) -> Result<XdrArchive<TransactionHistoryEntry>, Error> {
		let (url, file_name) = Self::get_url_and_file_name(slot_index);
		//try to find xdr.gz file and decode. if error then download archive from horizon archive
		// node and save
		println!("{url}");
		println!("{file_name}");
		let result = Self::try_gz_decode_archive_file(&file_name);

		if result.is_err() {
			let result = Self::download_file_and_save(&url, &file_name).await;
			if result.is_ok() {
				let data = Self::try_gz_decode_archive_file(&file_name)?;
				return Ok(Self::decode_xdr(data))
			}
		}
		let data = result.unwrap();
		Ok(Self::decode_xdr(data))
	}

	fn decode_xdr(xdr_data: Vec<u8>) -> XdrArchive<TransactionHistoryEntry> {
		XdrArchive::<TransactionHistoryEntry>::from_xdr(xdr_data).unwrap()
	}

	async fn download_file_and_save(url: &str, file_name: &str) -> Result<(), Error> {
		let response = reqwest::get(url).await.unwrap();
		let content = response.bytes().await.unwrap();

		let mut file = match File::create(&file_name) {
			Err(why) => panic!("couldn't create {}", why),
			Ok(file) => file,
		};
		file.write_all(content.as_bytes_ref())?;
		Ok(())
	}

	fn try_gz_decode_archive_file(path: &str) -> Result<Vec<u8>, Error> {
		use flate2::bufread::GzDecoder;
		use std::io::{self, BufReader, Read};
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
		let base_url = crate::oracle::constants::stellar_history_base_url_transactions;
		let url = format!(
			"{base_url}{}/{}/{}/transactions-{file_name}.gz",
			&hex_string[..2],
			&hex_string[2..4],
			&hex_string[4..6]
		);
		(url, format!("txs-{file_name}"))
	}

	fn find_last_slot_index_in_batch(slot_index: i32) -> i32 {
		let rest = (slot_index + 1) % 64;
		if rest == 0 {
			return slot_index
		}
		return slot_index + 64 - rest
	}

	fn read_file_xdr(filename: &str) -> Result<Vec<u8>, Error> {
		let mut file = File::open(filename)?;
		let mut bytes: Vec<u8> = vec![];
		file.read_to_end(&mut bytes)?;
		Ok(bytes)
	}
}

#[cfg(not(test))]
pub fn prepare_directories() -> Result<(), Error> {
	create_dir_all("./scp_envelopes")?;
	create_dir_all("./tx_sets")?;

	create_dir_all("./tx_hashes").map_err(Error::from)
}

#[cfg(test)]
pub fn prepare_directories() -> Result<(), Error> {
	create_dir_all("./resources/test/scp_envelopes")?;
	create_dir_all("./resources/test/tx_sets")?;

	create_dir_all("./resources/test/tx_hashes").map_err(Error::from)
}

#[cfg(test)]
mod test {
	use crate::oracle::{
		collector::get_tx_set_hash,
		constants::MAX_SLOTS_PER_FILE,
		errors::Error,
		storage::{
			traits::{FileHandler, FileHandlerExt},
			EnvelopesFileHandler, TxHashesFileHandler, TxSetsFileHandler,
		},
		types::Slot, TransactionsArchiveStorage,
	};
	use frame_support::assert_err;
	use mockall::lazy_static;
	use std::{convert::TryFrom, env, fs, fs::File, io::Read, path::PathBuf};
	use stellar_relay::{
		helper::compute_non_generic_tx_set_content_hash,
		sdk::{network::PUBLIC_NETWORK, types::ScpStatementPledges},
	};

	lazy_static! {
		static ref M_SLOTS_FILE: Slot =
			Slot::try_from(MAX_SLOTS_PER_FILE - 1).expect("should convert just fine");
	}

	#[test]
	fn find_file_by_slot_success() {
		// ---------------- TESTS FOR ENVELOPES  -----------
		// finding first slot
		{
			let slot = 573112;
			let expected_name = format!("{}_{}", slot, slot + *M_SLOTS_FILE);
			let file_name =
				EnvelopesFileHandler::find_file_by_slot(slot).expect("should return a file");
			assert_eq!(&file_name, &expected_name);
		}

		// finding slot in the middle of the file
		{
			let first_slot = 573312;
			let expected_name = format!("{}_{}", first_slot, first_slot + *M_SLOTS_FILE);
			let slot = first_slot + 5;

			let file_name =
				EnvelopesFileHandler::find_file_by_slot(slot).expect("should return a file");
			assert_eq!(&file_name, &expected_name);
		}

		// finding slot at the end of the file
		{
			let slot = 578490;
			let expected_name = format!("{}_{}", slot - *M_SLOTS_FILE, slot);

			let file_name =
				EnvelopesFileHandler::find_file_by_slot(slot).expect("should return a file");
			assert_eq!(&file_name, &expected_name);
		}

		// ---------------- TESTS FOR TX SETS  -----------
		// finding first slot
		{
			let slot = 42867088;
			let expected_name = format!("{}_42867102", slot);
			let file_name =
				TxSetsFileHandler::find_file_by_slot(slot).expect("should return a file");
			assert_eq!(&file_name, &expected_name);
		}

		// finding slot in the middle of the file
		{
			let first_slot = 42867103;
			let expected_name = format!("{}_42867118", first_slot);
			let slot = first_slot + 10;

			let file_name =
				TxSetsFileHandler::find_file_by_slot(slot).expect("should return a file");
			assert_eq!(&file_name, &expected_name);
		}

		// finding slot at the end of the file
		{
			let slot = 42867134;
			let expected_name = format!("42867119_{}", slot);

			let file_name =
				TxSetsFileHandler::find_file_by_slot(slot).expect("should return a file");
			assert_eq!(&file_name, &expected_name);
		}
	}

	#[test]
	fn get_map_from_archives_success() {
		// ---------------- TESTS FOR ENVELOPE  -----------
		{
			let first_slot = 578291;
			let last_slot = first_slot + *M_SLOTS_FILE;
			let envelopes_map = EnvelopesFileHandler::get_map_from_archives(last_slot - 20)
				.expect("should return envelopes map");

			for (idx, slot) in envelopes_map.keys().enumerate() {
				let expected_slot_num =
					first_slot + u64::try_from(idx).expect("should return u64 data type");
				assert_eq!(slot, &expected_slot_num);
			}

			let scp_envelopes = envelopes_map.get(&last_slot).expect("should have scp envelopes");
			for x in scp_envelopes {
				assert_eq!(x.statement.slot_index, last_slot);
			}
		}

		// ---------------- TEST FOR TXSETs  -----------
		{
			let first_slot = 42867119;
			let find_slot = first_slot + 15;
			let txsets_map = TxSetsFileHandler::get_map_from_archives(find_slot)
				.expect("should return txsets map");

			assert!(txsets_map.get(&find_slot).is_some());
		}
	}

	#[test]
	fn get_map_from_archives_fail() {
		// ---------------- TESTS FOR ENVELOPE  -----------
		{
			let slot = 578491;

			match EnvelopesFileHandler::get_map_from_archives(slot).expect_err("This should fail") {
				Error::Other(err_str) => {
					assert_eq!(err_str, format!("Cannot find file for slot {}", slot))
				},
				_ => assert!(false, "should fail"),
			}
		}

		// ---------------- TEST FOR TXSETs  -----------
		{
			let slot = 42867087;

			match TxSetsFileHandler::get_map_from_archives(slot).expect_err("This should fail") {
				Error::Other(err_str) => {
					assert_eq!(err_str, format!("Cannot find file for slot {}", slot))
				},
				_ => assert!(false, "should fail"),
			}
		}
	}

	#[test]
	fn write_to_file_success() {
		// ---------------- TESTS FOR ENVELOPE  -----------
		{
			let first_slot = 42867088;
			let last_slot = 42867102;

			let mut path = PathBuf::new();
			path.push("./resources/test/scp_envelopes_for_testing");
			path.push(&format!("{}_{}", first_slot, last_slot));

			let mut file = File::open(path).expect("file should exist");
			let mut bytes: Vec<u8> = vec![];
			let _ = file.read_to_end(&mut bytes).expect("should be able to read until the end");

			let mut env_map =
				EnvelopesFileHandler::deserialize_bytes(bytes).expect("should generate a map");

			// let's remove the first_slot and last_slot in the map, so we can create a new file.
			env_map.remove(&first_slot);
			env_map.remove(&last_slot);

			let expected_filename = format!("{}_{}", first_slot + 1, last_slot - 1);
			let actual_filename = EnvelopesFileHandler::write_to_file(&env_map)
				.expect("should write to scp_envelopes directory");
			assert_eq!(actual_filename, expected_filename);

			let new_file = EnvelopesFileHandler::find_file_by_slot(first_slot + 2)
				.expect("should return the same file");
			assert_eq!(new_file, expected_filename);

			// let's delete it
			let path = EnvelopesFileHandler::get_path(&new_file);
			fs::remove_file(path).expect("should be able to remove the newly added file.");
		}

		// ---------------- TEST FOR TXSETs  -----------
		{
			let first_slot = 42867151;
			let last_slot = 42867166;
			let mut path = PathBuf::new();
			path.push("./resources/test/tx_sets_for_testing");
			path.push(&format!("{}_{}", first_slot, last_slot));

			let mut file = File::open(path).expect("file should exist");
			let mut bytes: Vec<u8> = vec![];
			let _ = file.read_to_end(&mut bytes).expect("should be able to read until the end");

			let mut txset_map =
				TxSetsFileHandler::deserialize_bytes(bytes).expect("should generate a map");

			// let's remove the first_slot and last_slot in the map, so we can create a new file.
			txset_map.remove(&first_slot);
			txset_map.remove(&last_slot);

			let expected_filename = format!("{}_{}", first_slot + 1, last_slot - 1);
			let actual_filename = TxSetsFileHandler::write_to_file(&txset_map)
				.expect("should write to scp_envelopes directory");
			assert_eq!(actual_filename, expected_filename);

			let new_file = TxSetsFileHandler::find_file_by_slot(last_slot - 2)
				.expect("should return the same file");
			assert_eq!(new_file, expected_filename);

			// let's delete it
			let path = TxSetsFileHandler::get_path(&new_file);
			fs::remove_file(path).expect("should be able to remove the newly added file.");
		}
	}
	#[tokio::test]
	async fn get_scp_archive_works() {
		use super::ScpArchiveStorage;
		use std::convert::TryInto;
		use stellar_relay::sdk::types::ScpHistoryEntry;

		let slot_index = 30511500;

		let scp_archive = ScpArchiveStorage::get_scp_archive(slot_index)
			.await
			.expect("should find the archive");

		let slot_index_u32: u32 = slot_index.try_into().unwrap();
		scp_archive
			.get_vec()
			.into_iter()
			.find(|&scp_entry| {
				if let ScpHistoryEntry::V0(scp_entry_v0) = scp_entry {
					return scp_entry_v0.ledger_messages.ledger_seq == slot_index_u32
				} else {
					return false
				}
			})
			.expect("slot index should be in archive");
	}

	#[tokio::test]
	async fn get_transactions_archive_works() {
		use super::TransactionsArchiveStorage;
		use std::convert::TryInto;
		use stellar_relay::sdk::types::TransactionHistoryEntry;

		let slot_index = 30511500;

		let transactions_archive = TransactionsArchiveStorage::get_transactions_archive(slot_index)
			.await
			.expect("should find the archive");

		// println!("{:#?}", transactions_archive);

		// let slot_index_u32: u32 = slot_index.try_into().unwrap();
		// scp_archive
		// 	.get_vec()
		// 	.into_iter()
		// 	.find(|&scp_entry| {
		// 		if let ScpHistoryEntry::V0(scp_entry_v0) = scp_entry {
		// 			return scp_entry_v0.ledger_messages.ledger_seq == slot_index_u32
		// 		} else {
		// 			return false
		// 		}
		// 	})
		// 	.expect("slot index should be in archive");
	}
}
