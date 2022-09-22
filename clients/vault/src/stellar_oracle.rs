use crate::{
    error::Error
};
use substrate_stellar_sdk::{Hash};
use substrate_stellar_sdk::types::{ScpEnvelope, TransactionSet};

pub type Slot = Uint64;
pub type TxSetHash = Hash;
pub type TxHash = Hash;
pub type SerializedData = Vec<u8>;

/// For easy writing to file. BTreeMap to preserve order of the slots.
pub type SlotEncodedMap = BTreeMap<Slot,SerializedData>;

/// The slot is not found in the `StellarMessage::TxSet(...)`, therefore this map
/// serves as a holder of the slot when we hash the txset.
pub type TxSetCheckerMap = HashMap<TxSetHash,Slot>;

/// Todo: these maps should be merged into one; but there was a complication (which differs in every run):
/// Sometimes not enough `StellarMessage::ScpMessage(...)` are sent per slot;
/// or that the `Stellar:message::TxSet(...)` took too long to arrive (may not even arrive at all)
/// So I've kept both of them separate.
pub type EnvelopesMap = BTreeMap<Slot,Vec<ScpEnvelope>>;
pub type TxSetMap = BTreeMap<Slot,TransactionSet>;

pub type TxHashMap = (String,HashMap<TxHash, Slot>);

/// This is for `EnvelopesMap`; how many slots is accommodated per file.
pub const MAX_SLOTS_PER_FILE: Uint64 = 5;

/// This is for `EnvelopesMap`. Make sure that we have a minimum set of envelopes per slot,
/// before writing to file.
pub const MIN_EXTERNALIZED_MESSAGES: usize = 10;

/// This is both for `TxSetMap` and `TxHashMap`.
/// When the map reaches the MAX or more, then we write to file.
pub const MAX_TXS_PER_FILE:Uint64 = 3000;

/// trait to read/write the maps to a file.
pub trait FileHandler<T>{
    const PATH: &'static str; // path of where to store the file
    fn write_to_file(value:Self) -> Result<String, Error>;
    fn read_file(filename:String) -> T;
    fn find_file_by_slot(slot_param: Slot) -> Result<String,Error>;

    fn get_path(filename:&str) -> PathBuf {
        let mut path = PathBuf::new();
        path.push(Self::PATH);
        path.push(filename);

        path
    }
}

impl FileHandler<Self> for EnvelopesMap {
    const PATH: &'static str = "./scp_envelopes";

    /// Writes the EnvelopesMap into file.
    /// Returns the filename of the file.
    /// the filename format would be: <starting_slot_number>_<time_created>.json
    fn write_to_file(value: Self) -> Result<String, Error> {
        let mut filename: String = "".to_string();

        // convert the ScpEnvelope into a structure that can be serialized;
        // and that would be turning it to an xdr (or Vec<u8>)
        let mut m: SlotEncodedMap = SlotEncodedMap::new();

        for (idx, (key, value)) in value.into_iter().enumerate() {
            // retrieves the starting slot number and used as filename.
            if idx == 0 {
                filename.push_str(&format!("{}_{}.json", key, time_now()));
            }

            // vec cannot be converted to xdr; hence wrapping it inside an UnlimitedVarArray.
            let stellar_array = UnlimitedVarArray::new(value)?;
            m.insert(key, stellar_array.to_xdr());
        }

        // let's serialize this further with bincode
        let res = bincode::serialize(&m)?;

        // save to file
        let mut path = Self::get_path(&filename);
        let mut file = File::create(path)?;
        file.write_all(&res)?;

        Ok(filename)
    }

    fn read_file(filename: String) -> EnvelopesMap {
        let mut m: EnvelopesMap = EnvelopesMap::new();

        let mut path = Self::get_path(&filename);

        if let Ok(mut file) = File::open(path) {
            let mut bytes: Vec<u8> = vec![];
            let read_size = file.read_to_end(&mut bytes).unwrap_or(0);

            if read_size > 0 {
                let inside: SlotEncodedMap = bincode::deserialize(&bytes)
                    .unwrap_or(SlotEncodedMap::new());

                for (key, value) in inside.into_iter() {
                    if let Ok(envelopes) =
                    UnlimitedVarArray::<ScpEnvelope>::from_xdr(value) {
                        m.insert(key,envelopes.get_vec().to_vec());
                    }
                }
            }
        }

        m
    }

    fn find_file_by_slot(slot_param: Slot) -> Result<String,Error> {
        let paths = fs::read_dir(Self::PATH)
            .map_err(|e| Error::Undefined(e.to_string()))?;

        for path in paths {
            let filename = path.map_err(|e| Error::Undefined(e.to_string()))?
                .file_name().into_string().unwrap();

            let mut splits = filename.split("_");

            // we only want the first split, since that's where the slot is.
            if let Some(slot) = splits.next() {
                let slot_num = slot.parse::<Uint64>()?;

                // check if the slot we want is in this file.
                if slot_param >= slot_num && slot_param <= slot_num + MAX_SLOTS_PER_FILE {
                    // we found it! return this one
                    return Ok(filename);
                }
            }
        }

        Err(Error::Other(format!("Cannot find file for slot {}", slot_param)))
    }
}
