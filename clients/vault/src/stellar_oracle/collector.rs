use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    sync::Arc,
};

use parking_lot::{
    lock_api::{RwLockReadGuard, RwLockWriteGuard},
    RawRwLock, RwLock,
};
use stellar_relay::sdk::{
    types::{ScpEnvelope, ScpStatementExternalize, ScpStatementPledges, StellarMessage, TransactionSet},
    Memo, TransactionEnvelope,
};

use crate::stellar_oracle::{
    handler, EnvelopesFileHandler, EnvelopesMap, Error, FileHandlerExt, FilterWith, Slot, TxHash, TxHashMap,
    TxHashesFileHandler, TxSetCheckerMap, TxSetHash, TxSetMap, TxSetsFileHandler, MAX_SLOTS_PER_FILE, MAX_TXS_PER_FILE,
    MIN_EXTERNALIZED_MESSAGES,
};
use stellar_relay::{
    helper::compute_non_generic_tx_set_content_hash,
    sdk::network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
    UserControls,
};

pub struct ScpMessageCollector {
    /// holds the mapping of the Slot Number(key) and the ScpEnvelopes(value)
    envelopes_map: Arc<RwLock<EnvelopesMap>>,
    /// holds the mapping of the Slot Number(key) and the TransactionSet(value)
    txset_map: Arc<RwLock<TxSetMap>>,
    /// holds the mapping of the Transaction Hash(key) and the Slot Number(value)
    tx_hash_map: Arc<RwLock<TxHashMap>>,
    /// Holds the transactions that still have to be processed but were not because not enough scp messages are
    /// available yet.
    pending_transactions: Vec<TransactionEnvelope>,
    public_network: bool,
}

/// Read Implementation
impl ScpMessageCollector {
    pub fn envelopes_map(&self) -> RwLockReadGuard<'_, RawRwLock, EnvelopesMap> {
        self.envelopes_map.read()
    }

    pub fn txset_map(&self) -> RwLockReadGuard<'_, RawRwLock, TxSetMap> {
        self.txset_map.read()
    }

    pub fn tx_hash_map(&self) -> RwLockReadGuard<'_, RawRwLock, TxHashMap> {
        self.tx_hash_map.read()
    }

    pub fn network(&self) -> &Network {
        if self.public_network {
            &PUBLIC_NETWORK
        } else {
            &TEST_NETWORK
        }
    }

    pub fn is_public(&self) -> bool {
        self.public_network
    }
}

impl ScpMessageCollector {
    pub fn new(public_network: bool) -> Self {
        ScpMessageCollector {
            envelopes_map: Default::default(),
            txset_map: Default::default(),
            tx_hash_map: Default::default(),
            pending_transactions: vec![],
            public_network,
        }
    }

    pub fn envelopes_map_mut(&mut self) -> RwLockWriteGuard<'_, RawRwLock, EnvelopesMap> {
        self.envelopes_map.write()
    }

    pub fn txset_map_mut(&mut self) -> RwLockWriteGuard<'_, RawRwLock, TxSetMap> {
        self.txset_map.write()
    }

    pub fn tx_hash_map_mut(&mut self) -> RwLockWriteGuard<'_, RawRwLock, TxHashMap> {
        self.tx_hash_map.write()
    }

    /// handles incoming ScpEnvelope.
    ///
    /// # Arguments
    ///
    /// * `env` - the ScpEnvelope
    /// * `txset_hash_map` - provides the slot number of the given Transaction Set Hash
    /// * `user` - The UserControl used for sending messages to Stellar Node
    pub(crate) async fn handle_envelope(
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
                !self.txset_map().contains_key(&slot)
            {
                // we're creating a new entry
                txset_hash_map.insert(txset_hash, slot);
                user.send(StellarMessage::GetTxSet(txset_hash)).await?;

                // check if we need to write to file
                self.check_write_envelopes_to_file()?;
            }

            // insert/add messages
            let mut envelopes_map = self.envelopes_map_mut();

            if let Some(value) = envelopes_map.get_mut(&slot) {
                value.push(env);
            } else {
                tracing::info!("Adding received SCP envelopes for slot {}", slot);
                envelopes_map.insert(slot, vec![env]);
            }
        }

        Ok(())
    }

    /// checks whether the envelopes map requires saving to file.
    fn check_write_envelopes_to_file(&mut self) -> Result<(), Error> {
        let env_map = self.envelopes_map().clone();
        let mut keys = env_map.keys();
        let keys_len = u64::try_from(keys.len()).unwrap_or(0);

        // map is too small; we don't have to write it to file just yet.
        if keys_len < MAX_SLOTS_PER_FILE {
            return Ok(());
        }

        tracing::info!("The map is getting big. Let's write to file:");

        let mut counter = 0;
        while let Some(key) = keys.next() {
            // save to file if all data for the corresponding slots have been filled.
            if counter == MAX_SLOTS_PER_FILE {
                self.write_envelopes_to_file(*key)?;
                break;
            }

            if let Some(value) = env_map.get(key) {
                // check if we have enough externalized messages for the corresponding key
                if value.len() < MIN_EXTERNALIZED_MESSAGES && keys_len < MAX_SLOTS_PER_FILE * 5 {
                    tracing::info!("slot: {} not enough messages. Let's wait for more.", key);
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
        let new_slot_map = self.envelopes_map_mut().split_off(&last_slot);
        let _ = EnvelopesFileHandler::write_to_file(&self.envelopes_map())?;

        self.envelopes_map = Arc::new(RwLock::new(new_slot_map));
        tracing::info!("start slot is now: {:?}", last_slot);

        Ok(())
    }

    /// handles incoming TransactionSet.
    ///
    /// # Arguments
    ///
    /// * `set` - the TransactionSet
    /// * `txset_hash_map` - provides the slot number of the given Transaction Set Hash
    pub(crate) async fn handle_tx_set(
        &mut self,
        set: &TransactionSet,
        txset_hash_map: &mut TxSetCheckerMap,
        filter: &impl FilterWith,
    ) -> Result<(), Error> {
        self.check_write_tx_set_to_file()?;

        // compute the tx_set_hash, to check what slot this set belongs too.
        let tx_set_hash = compute_non_generic_tx_set_content_hash(set);

        if let Some(slot) = txset_hash_map.remove(&tx_set_hash) {
            self.txset_map_mut().insert(slot, set.clone());
            self.update_tx_hash_map(slot, set, filter).await?;
        } else {
            tracing::info!("WARNING! tx_set_hash: {:?} has no slot.", tx_set_hash);
        }

        Ok(())
    }

    /// checks whether the transaction set map requires saving to file.
    fn check_write_tx_set_to_file(&mut self) -> Result<(), Error> {
        // map is too small; we don't have to write it to file just yet.
        if self.tx_hash_map().len() < usize::try_from(MAX_TXS_PER_FILE).unwrap_or(0) {
            return Ok(());
        }

        tracing::info!("saving old transactions to file: {:?}", self.txset_map().keys());

        let filename = TxSetsFileHandler::write_to_file(&self.txset_map())?;

        TxHashesFileHandler::write_to_file(filename, &self.tx_hash_map())?;

        self.txset_map = Arc::new(RwLock::new(TxSetMap::new()));
        self.tx_hash_map = Arc::new(RwLock::new(HashMap::new()));

        Ok(())
    }

    /// tries to send validation proofs for pending transactions
    async fn check_pending_tx(&mut self) -> Result<(), Error> {
        // Store the handled transaction indices in a vec to be able to remove them later
        let mut handled_tx_indices = Vec::new();
        for (index, tx_env) in self.pending_transactions.iter().enumerate() {
            let handled = handler::handle_tx(tx_env.clone(), self).await?;
            if handled {
                handled_tx_indices.push(index);
            }
        }
        // Remove the handled transactions from the pending transactions
        for index in handled_tx_indices.iter().rev() {
            self.pending_transactions.remove(*index);
        }
        Ok(())
    }

    fn is_save_tx(&mut self, tx_env: &TransactionEnvelope, tx_hash: &TxHash, filter: &impl FilterWith) -> bool {
        match tx_env {
            TransactionEnvelope::EnvelopeTypeTxV0(value) => check_memo(&value.tx.memo),
            TransactionEnvelope::EnvelopeTypeTx(value) => {
                if filter.is_tx_relevant(&value.tx) &&
                    // Add transaction to pending transactions if it is not yet contained
                    self.pending_transactions.iter()
                        .find(|tx| &tx.get_hash(self.network()) == tx_hash).is_none()
                {
                    self.pending_transactions.push(tx_env.clone());
                }
                check_memo(&value.tx.memo)
            }
            TransactionEnvelope::EnvelopeTypeTxFeeBump(_) => false,
            TransactionEnvelope::Default(code) => {
                tracing::info!("Default: {:?}", code);
                false
            }
        }
    }

    /// maps the slot to the transactions of the TransactionSet
    async fn update_tx_hash_map(
        &mut self,
        slot: Slot,
        tx_set: &TransactionSet,
        filter_with: &impl FilterWith,
    ) -> Result<(), Error> {
        tracing::info!("Inserting received transaction set for slot {}", slot);

        // Collect tx hashes to build proofs, and transactions to validate
        tx_set.txes.get_vec().iter().for_each(|tx_env| {
            let tx_hash = tx_env.get_hash(self.network());

            if self.is_save_tx(tx_env, &tx_hash, filter_with) {
                self.tx_hash_map_mut().insert(tx_hash, slot);
            }
        });

        self.check_pending_tx().await?;

        Ok(())
    }
}

pub fn get_tx_set_hash(x: &ScpStatementExternalize) -> Result<TxSetHash, Error> {
    let scp_value = x.commit.value.get_vec();
    scp_value[0..32].try_into().map_err(Error::from)
}

fn check_memo(memo: &Memo) -> bool {
    match memo {
        Memo::MemoHash(_) => true,
        _ => false,
    }
}
