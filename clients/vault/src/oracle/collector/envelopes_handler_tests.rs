use std::collections::HashSet;
use std::convert::TryFrom;
use std::path::PathBuf;
use mockall::lazy_static;
use rand::Rng;
use crate::oracle::collector::ScpMessageCollector;
use crate::oracle::storage::EnvelopesFileHandler;
use crate::oracle::storage::traits::FileHandler;
use crate::oracle::Slot;
use crate::oracle::constants::MAX_SLOTS_PER_FILE;

lazy_static!{
     static ref M_SLOTS_FILE:Slot = Slot::try_from(MAX_SLOTS_PER_FILE-1).expect("should convert just fine");
 }

#[test]
fn check_write_envelopes_to_file_false() {

    let first_slot = 578291;
    let last_slot = first_slot + *M_SLOTS_FILE;

    let mut env_map = EnvelopesFileHandler::get_map_from_archives(first_slot + 3).expect("should return a map");

    // let's remove some elements
    let mut rng = rand::thread_rng();
    let rand_num_of_elements = rng.gen_range(1..(*M_SLOTS_FILE / 2));
    let rand_num_of_elements = usize::try_from(rand_num_of_elements).expect("should convert ok");

    let res = rng.sample_iter(rand::distributions::Uniform::from(first_slot..last_slot)).take(rand_num_of_elements).collect::<HashSet<Slot>>();

    res.iter().for_each(|slot| {
        assert!(env_map.remove(slot).is_some());
    });

    let mut collector = ScpMessageCollector::new(true, vec![]);
    collector.envelopes_map_mut().append(&mut env_map);

    // this should not write to file.
    let res = collector.check_write_envelopes_to_file(last_slot + 1).expect("should not fail");
    assert!(!res);

}