# Oracle

The Stellar Oracle uses [Stellar-Relay](../stellar-relay-lib) to listen to messages from the Stellar Node.  
The Oracle collects and saves **`SCPStatementExternalize`** SCPMessages and its corresponding `TransactionSets`.

## Usage

### Provide the `StellarRelayConfig` and _secret key_
Refer to [Stellar-Relay readme](../stellar-relay-lib/README.md#the-stellaroverlayconfig) for the `StellarRelayConfig`.

### Start the `OracleAgent`
Simply call the _async_ _`start_oracle_agent()`_ function of `OracleAgent` with the config and secret key as parameters:
```rust
let mut oracle_agent = start_oracle_agent(<stellar_relay_config>,<secret_key>).await;
```
Note: starting the agent means listening to messages coming from  the `StellarOverlayConnection`.
### Building a proof
Given a slot, call the method: 
```rust
oracle_agent.get_proof(<slot>).await
```

### Stopping the `OracleAgent`
It is as simple as:
```rust
oracle_agent.stop()
```

## Understanding `OracleAgent`'s field: `collector`
The `collector` is a `ScpMessageCollector` that stores the ScpMessages and its corresponding TransactionSet.
```rust
pub struct ScpMessageCollector {
	/// holds the mapping of the Slot Number(key) and the ScpEnvelopes(value)
	envelopes_map: Arc<RwLock<EnvelopesMap>>,

	/// holds the mapping of the Slot Number(key) and the TransactionSet(value)
	txset_map: Arc<RwLock<TxSetMap>>,

	/// Mapping between the txset's hash and its corresponding slot.
	/// An entry is removed when a `TransactionSet` is found.
	txset_and_slot_map: Arc<RwLock<TxSetHashAndSlotMap>>,

	last_scp_ext_slot: Arc<RwLock<u64>>,

	public_network: bool,
}
```
The `ScpMessageCollector` has methods such as `add_scp_envelopes` and `add_txset` to store the ScpMessages and TransactionSet respectively.

### How `ScpMessageCollector` handles `ScpEnvelopes` and `TransactionSet` 
Found in [handler.rs](src/oracle/collector/handler.rs) contains 2 methods:
* _`async fn handle_envelope( &self, env: ScpEnvelope, message_sender: &StellarMessageSender,)`_
  * handles only envelopes with `ScpStExternalize`, and saves two important things in the **`txset_and_slot_map`** field:
    * extracting the _slot_ from this envelope for future reference
    * extracting the _txset_hash_.
  * sends a `GetTxSet(<txset_hash>)` message to Stellar Relay (on the first occurrence of this slot).
* _`handle_tx_set(&self, set: TransactionSet)`_
  * handles only `TransactionSet`s that are in the **`txset_and_slot_map`** field.
  * given a `TransactionSet`, extract the `txset_hash` and check if it's one of the `txset_hash`es we want.
    * if it is, save to **`txset_map`** field.

### How `ScpMessageCollector` builds the `Proof`
```rust
pub struct Proof {
	/// the slot (or ledger) where the transaction is found
	slot: Slot,

	/// the envelopes belonging to the slot
	envelopes: UnlimitedVarArray<ScpEnvelope>,

	/// the transaction set belonging to the slot
	tx_set: TransactionSet,
}
```
* _`pub async fn build_proof(&self, slot: Slot, sender: &StellarMessageSender)`_
  * gets all the envelopes and the transaction set that belongs to the slot.
  * if any of these are not fulfilled, either:
    * ask the Stellar Relay for the envelopes and/or the transactionset; or
    * ask the archive for the envelopes and/or the transactionset
  * note: this method is called when calling OracleAgent's `get_proof(..)`.