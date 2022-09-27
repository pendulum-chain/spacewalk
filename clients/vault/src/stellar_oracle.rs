#![allow(dead_code)]

use serde::Serialize;
use std::{
    collections::{btree_map::Keys, BTreeMap, HashMap},
    convert::{TryFrom, TryInto},
    fs,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    str::Split,
};

use crate::error::Error;
use stellar_relay::{
    connect,
    helper::{compute_non_generic_tx_set_content_hash, time_now},
    node::NodeInfo, ConnConfig, StellarNodeMessage, UserControls,
};

use stellar_relay::sdk::{
    compound_types::UnlimitedVarArray,
    network::Network,
    types::{
        ScpEnvelope, ScpStatementExternalize, ScpStatementPledges, StellarMessage, TransactionSet, Uint64,
    },
    Hash, XdrCodec,
};

pub type Slot = Uint64;
pub type TxSetHash = Hash;
pub type TxHash = Hash;
pub type SerializedData = Vec<u8>;

/// For easy writing to file. BTreeMap to preserve order of the slots.
pub type SlotEncodedMap = BTreeMap<Slot, SerializedData>;

/// The slot is not found in the `StellarMessage::TxSet(...)`, therefore this map
/// serves as a holder of the slot when we hash the txset.
pub type TxSetCheckerMap = HashMap<TxSetHash, Slot>;

/// Todo: these maps should be merged into one; but there was a complication (which differs in every run):
/// Sometimes not enough `StellarMessage::ScpMessage(...)` are sent per slot;
/// or that the `Stellar:message::TxSet(...)` took too long to arrive (may not even arrive at all)
/// So I've kept both of them separate.
#[derive(Clone, Debug)]
pub struct EnvelopesMap(BTreeMap<Slot, Vec<ScpEnvelope>>);

#[derive(Clone, Debug)]
pub struct TxSetMap(BTreeMap<Slot, TransactionSet>);

pub type TxHashMap = (String, HashMap<TxHash, Slot>);

/// This is for `EnvelopesMap`; how many slots is accommodated per file.
pub const MAX_SLOTS_PER_FILE: Uint64 = 5;

/// This is for `EnvelopesMap`. Make sure that we have a minimum set of envelopes per slot,
/// before writing to file.
pub const MIN_EXTERNALIZED_MESSAGES: usize = 10;

/// This is both for `TxSetMap` and `TxHashMap`.
/// When the map reaches the MAX or more, then we write to file.
pub const MAX_TXS_PER_FILE: Uint64 = 3000;

impl EnvelopesMap {
    fn new() -> Self {
        let inner: BTreeMap<Slot, Vec<ScpEnvelope>> = BTreeMap::new();
        EnvelopesMap(inner)
    }

    fn insert(&mut self, key: Slot, value: Vec<ScpEnvelope>) -> Option<Vec<ScpEnvelope>> {
        self.0.insert(key, value)
    }

    fn keys(&self) -> Keys<'_, Slot, Vec<ScpEnvelope>> {
        self.0.keys()
    }

    fn get(&self, key: &Slot) -> Option<&Vec<ScpEnvelope>> {
        self.0.get(key)
    }

    fn get_mut(&mut self, key: &Slot) -> Option<&mut Vec<ScpEnvelope>> {
        self.0.get_mut(key)
    }

    fn split_off(&mut self, key: &Slot) -> Self {
        EnvelopesMap(self.0.split_off(key))
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

impl Default for EnvelopesMap {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl TxSetMap {
    fn new() -> Self {
        TxSetMap(BTreeMap::new())
    }

    fn contains_key(&self, key: &Slot) -> bool {
        self.0.contains_key(key)
    }

    fn insert(&mut self, key: Slot, value: TransactionSet) -> Option<TransactionSet> {
        self.0.insert(key, value)
    }

    fn keys(&self) -> Keys<'_, Slot, TransactionSet> {
        self.0.keys()
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

impl Default for TxSetMap {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

pub trait FileHandler<T: Default> {
    // path to where the file should be saved
    const PATH: &'static str;
    fn write_to_file(value: Self) -> Result<String, Error>;
    fn deserialize_bytes(bytes: Vec<u8>) -> Result<T, Error>;

    fn check_slot_in_splitted_filename(slot_param: Slot, splits: &mut Split<&str>) -> bool;

    fn get_path(filename: &str) -> PathBuf {
        let mut path = PathBuf::new();
        path.push(Self::PATH);
        path.push(filename);
        path
    }

    fn read_file(filename: String) -> Result<T, Error> {
        let path = Self::get_path(&filename);
        let mut file = File::open(path)?;

        let mut bytes: Vec<u8> = vec![];
        let read_size = file.read_to_end(&mut bytes)?;

        if read_size > 0 {
            return Self::deserialize_bytes(bytes);
        }

        Ok(T::default())
    }

    /// helper function for the impl of `write_to_file`
    fn _write_to_file<S: ?Sized + Serialize>(filename: &str, data: &S) -> Result<(), Error> {
        let res = bincode::serialize(data)?;

        let path = Self::get_path(filename);
        let mut file = File::create(path)?;

        file.write_all(&res).map_err(|e| Error::StdIoError(e))
    }

    fn find_file_by_slot(slot_param: Slot) -> Result<String, Error> {
        let paths = fs::read_dir(Self::PATH)?;

        for path in paths {
            let filename = path?.file_name().into_string().unwrap();
            let mut splits = filename.split("_");

            if Self::check_slot_in_splitted_filename(slot_param, &mut splits) {
                return Ok(filename);
            }
        }

        Err(Error::Other(format!("Cannot find file for slot {}", slot_param)))
    }
}

impl FileHandler<Self> for EnvelopesMap {
    const PATH: &'static str = "./scp_envelopes";

    fn write_to_file(value: Self) -> Result<String, Error> {
        let mut filename: String = "".to_string();
        let mut m: SlotEncodedMap = SlotEncodedMap::new();

        for (idx, (key, value)) in value.0.into_iter().enumerate() {
            if idx == 0 {
                filename.push_str(&format!("{}_{}.json", key, time_now()));
            }

            let stellar_array = UnlimitedVarArray::new(value)?; //.map_err(Error::from)?;
            m.insert(key, stellar_array.to_xdr());
        }

        Self::_write_to_file(&filename, &m)?;
        Ok(filename)
    }

    fn deserialize_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
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
        if let Some(slot) = splits.next() {
            println!("THE SLOT? {}", slot);

            match slot.parse::<Uint64>() {
                Ok(slot_num) if slot_param >= slot_num && slot_param <= slot_num + MAX_SLOTS_PER_FILE => {
                    // we found it! return this one
                    return true;
                }
                Ok(_) => {}
                Err(_) => {
                    println!("unconventional named file.");
                }
            }
        }

        false
    }
}

impl FileHandler<Self> for TxSetMap {
    const PATH: &'static str = "./tx_sets";

    fn write_to_file(value: Self) -> Result<String, Error> {
        let mut filename: String = "".to_string();
        let mut m: SlotEncodedMap = SlotEncodedMap::new();
        let len = value.len();

        for (idx, (key, set)) in value.0.into_iter().enumerate() {
            if idx == 0 {
                filename.push_str(&format!("{}_", key));
            }

            if idx == (len - 1) {
                filename.push_str(&format!("{}_{}.json", key, time_now()));
            }

            m.insert(key, set.to_xdr());
        }

        Self::_write_to_file(&filename, &m)?;
        Ok(filename)
    }

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

impl FileHandler<HashMap<Hash, Slot>> for TxHashMap {
    const PATH: &'static str = "./tx_hashes";

    fn write_to_file(value: Self) -> Result<String, Error> {
        Self::_write_to_file(&value.0, &value.1)?;
        Ok(value.0)
    }

    fn deserialize_bytes(bytes: Vec<u8>) -> Result<HashMap<Hash, Slot>, Error> {
        bincode::deserialize(&bytes).map_err(Error::from)
    }

    fn check_slot_in_splitted_filename(slot_param: Slot, splits: &mut Split<&str>) -> bool {
        TxSetMap::check_slot_in_splitted_filename(slot_param, splits)
    }
}

pub struct ScpMessageCollector {
    /// holds the mapping of the Slot Number(key) and the ScpEnvelopes(value)
    envelopes_map: EnvelopesMap,
    /// holds the mapping of the Slot Number(key) and the TransactionSet(value)
    txset_map: TxSetMap,
    /// holds the mapping of the Transaction Hash(key) and the Slot Number(value)
    tx_hash_map: HashMap<Hash, Slot>,
    network: Network,
}

impl ScpMessageCollector {
    pub fn new(network: Network) -> Self {
        ScpMessageCollector {
            envelopes_map: Default::default(),
            txset_map: Default::default(),
            tx_hash_map: Default::default(),
            network,
        }
    }

    /// handles incoming ScpEnvelope.
    ///
    /// # Arguments
    ///
    /// * `env` - the ScpEnvelope
    /// * `txset_hash_map` - provides the slot number of the given Transaction Set Hash
    /// * `user` - The UserControl used for sending messages to Stellar Node
    async fn handle_envelope(
        &mut self,
        env: ScpEnvelope,
        txset_hash_map: &mut TxSetCheckerMap,
        user: &UserControls,
    ) -> Result<(), Error> {
        let slot = env.statement.slot_index;

        // we are only interested with `ScpStExternalize`. Other messages are ignored.
        if let ScpStatementPledges::ScpStExternalize(stmt) = &env.statement.pledges {
            let txset_hash = get_tx_set_hash(stmt)?;

            if txset_hash_map.get(&txset_hash).is_none() &&
                // let's check whether this is a delayed message.
                !self.txset_map.contains_key(&slot)
            {
                // we're creating a new entry
                txset_hash_map.insert(txset_hash, slot);
                user.send(StellarMessage::GetTxSet(txset_hash)).await?;

                // check if we need to write to file
                self.check_write_envelopes_to_file()?;
            }

            // insert/add messages
            match self.envelopes_map.get_mut(&slot) {
                None => {
                    println!("slot: {} add to envelopes map", slot);

                    self.envelopes_map.insert(slot, vec![env]);
                }
                Some(value) => {
                    println!("slot: {} insert to envelopes map", slot);
                    value.push(env);
                }
            }
        }

        Ok(())
    }

    /// checks whether the envelopes map requires saving to file.
    fn check_write_envelopes_to_file(&mut self) -> Result<(), Error> {
        let mut keys = self.envelopes_map.keys();
        let keys_len = u64::try_from(keys.len()).unwrap_or(0);

        // map is too small; we don't have to write it to file just yet.
        if keys_len < MAX_SLOTS_PER_FILE {
            return Ok(());
        }

        println!("The map is getting big. Let's write to file:");

        let mut counter = 0;
        while let Some(key) = keys.next() {
            // save to file if all data for the corresponding slots have been filled.
            if counter == MAX_SLOTS_PER_FILE {
                self.write_envelopes_to_file(*key)?;
                break;
            }

            if let Some(value) = self.envelopes_map.get(key) {
                // check if we have enough externalized messages for the corresponding key
                if value.len() < MIN_EXTERNALIZED_MESSAGES && keys_len < MAX_SLOTS_PER_FILE * 5 {
                    println!("slot: {} not enough messages. Let's wait for more.", key);
                    break;
                }
            } else {
                // something wrong??? race condition?
                break;
            }

            counter += 1;
        }

        Ok(())
    }

    fn write_envelopes_to_file(&mut self, last_slot: Slot) -> Result<(), Error> {
        let new_slot_map = self.envelopes_map.split_off(&last_slot);
        EnvelopesMap::write_to_file(self.envelopes_map.clone())?;
        self.envelopes_map = new_slot_map;
        println!("start slot is now: {:?}", last_slot);

        Ok(())
    }

    /// handles incoming TransactionSet.
    ///
    /// # Arguments
    ///
    /// * `set` - the TransactionSet
    /// * `txset_hash_map` - provides the slot number of the given Transaction Set Hash
    fn handle_tx_set(&mut self, set: &TransactionSet, txset_hash_map: &mut TxSetCheckerMap) -> Result<(), Error> {
        self.check_write_tx_set_to_file()?;

        // compute the tx_set_hash, to check what slot this set belongs too.
        let tx_set_hash = compute_non_generic_tx_set_content_hash(set);

        if let Some(slot) = txset_hash_map.remove(&tx_set_hash) {
            self.txset_map.insert(slot, set.clone());
            self.update_tx_hash_map(slot, set);
        } else {
            println!("WARNING! tx_set_hash: {:?} has no slot.", tx_set_hash);
        }

        Ok(())
    }

    /// checks whether the transaction set map requires saving to file.
    fn check_write_tx_set_to_file(&mut self) -> Result<(), Error> {
        // map is too small; we don't have to write it to file just yet.
        if self.tx_hash_map.len() < usize::try_from(MAX_TXS_PER_FILE).unwrap_or(0) {
            return Ok(());
        }

        println!("saving old transactions to file: {:?}", self.txset_map.keys());

        let file_name = TxSetMap::write_to_file(self.txset_map.clone())?;
        TxHashMap::write_to_file((file_name, self.tx_hash_map.clone()))?;

        self.txset_map = TxSetMap::new();
        self.tx_hash_map = HashMap::new();

        Ok(())
    }

    /// maps the slot to the transactions of the TransactionSet
    fn update_tx_hash_map(&mut self, slot: Slot, set: &TransactionSet) {
        set.txes.get_vec().iter().for_each(|tx_env| {
            let tx_hash = tx_env.get_hash(&self.network);
            self.tx_hash_map.insert(tx_hash, slot);
        });
    }
}

fn get_tx_set_hash(x: &ScpStatementExternalize) -> Result<Hash, Error> {
    let scp_value = x.commit.value.get_vec();
    scp_value[0..32].try_into().map_err(Error::from)
}

pub async fn collect_scp_messages(collector:&mut ScpMessageCollector, node_info: NodeInfo, connection_cfg: ConnConfig) -> Result<(), Error> {
    fs::create_dir_all("./scp_envelopes")?;
    fs::create_dir_all("./tx_sets")?;
    fs::create_dir_all("./tx_hashes")?;

    // connecting to Stellar Node
    let mut user: UserControls = connect(node_info, connection_cfg).await?;


    // just a temporary holder
    let mut tx_set_hash_map: HashMap<Hash, Slot> = HashMap::new();

    while let Some(conn_state) = user.recv().await {
        match conn_state {
            StellarNodeMessage::Data { p_id, msg_type:_, msg } => match msg {
                StellarMessage::ScpMessage(env) => {
                    println!(
                        "pid: {} the map sizes: txset map: {}, envelopes map: {} txhash map: {}",
                        p_id,
                        collector.txset_map.len(),
                        collector.envelopes_map.len(),
                        collector.tx_hash_map.len()
                    );

                    collector.handle_envelope(env, &mut tx_set_hash_map, &user).await?;
                }
                StellarMessage::TxSet(set) => {
                    collector.handle_tx_set(&set, &mut tx_set_hash_map)?;
                }
                _ => {}
            },

            _ => {}
        }
    }
    Ok(())
}
