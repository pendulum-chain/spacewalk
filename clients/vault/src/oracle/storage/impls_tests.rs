use std::convert::TryFrom;
use std::{env, fs};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use frame_support::assert_err;
use mockall::lazy_static;
use stellar_relay::helper::compute_non_generic_tx_set_content_hash;
use stellar_relay::sdk::network::PUBLIC_NETWORK;
use stellar_relay::sdk::types::ScpStatementPledges;
use crate::oracle::collector::get_tx_set_hash;
use crate::oracle::constants::MAX_SLOTS_PER_FILE;
use crate::oracle::errors::Error;
use crate::oracle::storage::{EnvelopesFileHandler, TxHashesFileHandler, TxSetsFileHandler};
use crate::oracle::storage::traits::{FileHandler, FileHandlerExt};
use crate::oracle::types::Slot;

 lazy_static!{
     static ref M_SLOTS_FILE:Slot = Slot::try_from(MAX_SLOTS_PER_FILE-1).expect("should convert just fine");
 }

#[test]
fn find_file_by_slot_success() {

    // ---------------- TESTS FOR ENVELOPES  -----------
    // finding first slot
    {
        let slot = 573112;
        let expected_name = format!("{}_{}",slot,slot + *M_SLOTS_FILE);
        let file_name =
            EnvelopesFileHandler::find_file_by_slot(slot).expect("should return a file");
        assert_eq!(&file_name, &expected_name);
    }

    // finding slot in the middle of the file
    {
        let first_slot = 573312;
        let expected_name = format!("{}_{}",first_slot, first_slot + *M_SLOTS_FILE);
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
        let expected_name = format!("{}_42867102",slot);
        let file_name =
            TxSetsFileHandler::find_file_by_slot(slot).expect("should return a file");
        assert_eq!(&file_name, &expected_name);
    }

    // finding slot in the middle of the file
    {
        let first_slot = 42867103;
        let expected_name = format!("{}_42867118",first_slot);
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
        let envelopes_map = EnvelopesFileHandler::get_map_from_archives(last_slot - 20).expect("should return envelopes map");

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
        let txsets_map = TxSetsFileHandler::get_map_from_archives(find_slot).expect("should return txsets map");

        assert!(txsets_map.get(&find_slot).is_some());
    }
}

#[test]
fn get_map_from_archives_fail(){
    // ---------------- TESTS FOR ENVELOPE  -----------
    {
        let slot = 578491;

        match EnvelopesFileHandler::get_map_from_archives(slot).expect_err("This should fail") {
            Error::Other(err_str) => {
                assert_eq!(err_str, format!("Cannot find file for slot {}",slot))
            }
            _ => assert!(false, "should fail")
        }
    }

    // ---------------- TEST FOR TXSETs  -----------
    {
        let slot = 42867087;

        match TxSetsFileHandler::get_map_from_archives(slot).expect_err("This should fail") {
            Error::Other(err_str) => {
                assert_eq!(err_str, format!("Cannot find file for slot {}",slot))
            }
            _ => assert!(false, "should fail")
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
        path.push(&format!("{}_{}", first_slot,last_slot));

        let mut file = File::open(path).expect("file should exist");
        let mut bytes: Vec<u8> = vec![];
        let _ = file.read_to_end(&mut bytes).expect("should be able to read until the end");

        let mut env_map = EnvelopesFileHandler::deserialize_bytes(bytes).expect("should generate a map");

        // let's remove the first_slot and last_slot in the map, so we can create a new file.
        env_map.remove(&first_slot);
        env_map.remove(&last_slot);

        let expected_filename = format!("{}_{}",first_slot+1, last_slot-1);
        let actual_filename = EnvelopesFileHandler::write_to_file(&env_map).expect("should write to scp_envelopes directory");
        assert_eq!(actual_filename,expected_filename);

        let new_file = EnvelopesFileHandler::find_file_by_slot(first_slot+2).expect("should return the same file");
        assert_eq!(new_file,expected_filename);

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
        path.push(&format!("{}_{}", first_slot,last_slot));

        let mut file = File::open(path).expect("file should exist");
        let mut bytes: Vec<u8> = vec![];
        let _ = file.read_to_end(&mut bytes).expect("should be able to read until the end");

        let mut txset_map = TxSetsFileHandler::deserialize_bytes(bytes).expect("should generate a map");

        // let's remove the first_slot and last_slot in the map, so we can create a new file.
        txset_map.remove(&first_slot);
        txset_map.remove(&last_slot);


        let expected_filename = format!("{}_{}",first_slot+1, last_slot-1);
        let actual_filename = TxSetsFileHandler::write_to_file(&txset_map).expect("should write to scp_envelopes directory");
        assert_eq!(actual_filename,expected_filename);

        let new_file = TxSetsFileHandler::find_file_by_slot(last_slot-2).expect("should return the same file");
        assert_eq!(new_file,expected_filename);

        // let's delete it
        let path = TxSetsFileHandler::get_path(&new_file);
        fs::remove_file(path).expect("should be able to remove the newly added file.");
    }
}