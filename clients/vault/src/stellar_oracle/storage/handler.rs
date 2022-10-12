use std::{
    fs::{create_dir_all, File},
    io::Write,
    str::Split,
};
use stellar_relay::sdk::{
    compound_types::UnlimitedVarArray,
    types::{ScpEnvelope, TransactionSet},
};

use crate::stellar_oracle::{
    storage::traits::*, EnvelopesMap, Error, Filename, SerializedData, Slot, SlotEncodedMap, TxHashMap, TxSetMap,
};

use stellar_relay::sdk::XdrCodec;

pub struct EnvelopesFileHandler;

pub struct TxSetsFileHandler;

pub struct TxHashesFileHandler;

impl FileHandler<EnvelopesMap> for EnvelopesFileHandler {
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
                return (slot_param >= start_slot) && (slot_param <= end_slot);
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

            let stellar_array = UnlimitedVarArray::new(value.clone())?; //.map_err(Error::from)?;
            m.insert(*key, stellar_array.to_xdr());
        }

        let res = bincode::serialize(&m)?;

        Ok((filename, res))
    }
}

impl FileHandler<TxSetMap> for TxSetsFileHandler {
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
    const PATH: &'static str = "./tx_hashes";

    fn deserialize_bytes(bytes: Vec<u8>) -> Result<TxHashMap, Error> {
        bincode::deserialize(&bytes).map_err(Error::from)
    }

    fn check_slot_in_splitted_filename(slot_param: Slot, splits: &mut Split<&str>) -> bool {
        TxSetsFileHandler::check_slot_in_splitted_filename(slot_param, splits)
    }
}

pub fn prepare_directories() -> Result<(), Error> {
    create_dir_all("./scp_envelopes")?;
    create_dir_all("./tx_sets")?;

    create_dir_all("./tx_hashes").map_err(Error::from)
}
