use std::convert::TryFrom;
use std::env;

use stellar_relay::sdk::types::ScpStatementPledges;

use crate::oracle::traits::FileHandler;
use stellar_relay::helper::compute_non_generic_tx_set_content_hash;
use stellar_relay::sdk::network::Network;
use crate::oracle::collector::get_tx_set_hash;
use crate::oracle::storage::{EnvelopesFileHandler, TxHashesFileHandler, TxSetsFileHandler};

//todo: this needs update! the files don't exist anymore!
#[test]
fn find_file_successful() {
    let slot = 42867089;

    let file_name =
        EnvelopesFileHandler::find_file_by_slot(slot).expect("should return a file");
    assert_eq!(&file_name, "42867088_42867102");

    let file_name = TxSetsFileHandler::find_file_by_slot(slot).expect("should return a file");
    assert_eq!(&file_name, "42867088_42867102");

    let file_name = TxHashesFileHandler::find_file_by_slot(slot).expect("should return a file");
    assert_eq!(&file_name, "42867088_42867102");

    let slot = 42867150;

    let file_name =
        EnvelopesFileHandler::find_file_by_slot(slot).expect("should return a file");
    assert_eq!(&file_name, "42867148_42867162");

    let file_name = TxSetsFileHandler::find_file_by_slot(slot).expect("should return a file");
    assert_eq!(&file_name, "42867135_42867150");

    let file_name = TxHashesFileHandler::find_file_by_slot(slot).expect("should return a file");
    assert_eq!(&file_name, "42867135_42867150");

    let slot = 42990037;
    let file_name =
        EnvelopesFileHandler::find_file_by_slot(slot).expect("should return a file");
    assert_eq!(&file_name, "42990036_42990037");
}

#[test]
fn read_file_successful() {
    let first_slot = 42867118;
    let last_slot = 42867132;

    let envelopes_map =
        EnvelopesFileHandler::read_file(&format!("{}_{}", first_slot, last_slot))
            .expect("should return a map");

    for (idx, slot) in envelopes_map.keys().enumerate() {
        let expected_slot_num =
            first_slot + u64::try_from(idx).expect("should return u64 data type");
        assert_eq!(slot, &expected_slot_num);
    }

    let scp_envelopes = envelopes_map.get(&last_slot).expect("should have scp envelopes");

    for x in scp_envelopes {
        assert_eq!(x.statement.slot_index, last_slot);
    }

    let filename =
        TxSetsFileHandler::find_file_by_slot(last_slot).expect("should return a filename");
    let txset_map = TxSetsFileHandler::read_file(&filename).expect("should return a txset map");

    let txset = txset_map.get(&last_slot).expect("should have a txset");
    let tx_set_hash = compute_non_generic_tx_set_content_hash(txset);

    let first = scp_envelopes.first().expect("should return an envelope");

    if let ScpStatementPledges::ScpStExternalize(stmt) = &first.statement.pledges {
        let expected_tx_set_hash = get_tx_set_hash(&stmt).expect("return a tx set hash");

        assert_eq!(tx_set_hash, expected_tx_set_hash);
    } else {
        assert!(false);
    }

    let txes = txset.txes.get_vec();
    let idx = txes.len() / 2;
    let tx = txes.get(idx).expect("should return a tx envelope.");

    let network = Network::new(b"Public Global Stellar Network ; September 2015");

    let hash = tx.get_hash(&network);
    let txhash_map =
        TxHashesFileHandler::read_file(&filename).expect("should return txhash map");
    let actual_slot = txhash_map.get(&hash).expect("should return a slot number");
    assert_eq!(actual_slot, &last_slot);
}
