# Oracle

The Stellar Oracle uses [Stellar-Relay](../stellar-relay) to listen to messages from the StellarNode.  
The Oracle collects and saves **`SCPStatementExternalize`** SCPMessages and its corresponding `TransactionSets`.

## Usage

### Provide the `NodeInfo` and `ConnConfig` 
Refer to [Stellar-Relay readme](../stellar-relay/README.md) on how to initialize these structures.

### Create the `ScpMessageHandler`
Simply call the _async_ function `create_handler()`:
```rust
let handler = create_handler(node_info, cfg, is_public_network, vault_addresses_filter).await?;
```
The `vault_address_filter` is a list of addresses for the vault.

The `ScpMessageHandler` has a couple of async methods available:
* _`get_size()`_ -> returns the number of slots saved in the map at runtime.
* _`add_filter(filter: Box<TxEnvelopeFilter>)`_ -> adds a unique filter that will check if a given transaction should be processed.
* _`remove_filter(filter_id)`_ -> removes a filter
* _`get_pending_txs()`_ -> gets a list of transaction proofs

## The `FilterWith` trait
```rust
pub trait FilterWith<T> {
    /// unique identifier of this filter
    fn id(&self) -> u32;

    /// logic to check whether a given param should be processed.
    fn check_for_processing(&self, param: &T) -> bool;
}
```
The oracle looks through all the transactions in a `TransactionSet` and checks if a transaction is to be processed.
In the case of `RequestIssueEvent`, we can do something like:
```rust
pub struct IssueEventsFilter{
    issue_ids: Vec<stellar_relay::sdk::Hash>
}

impl FilterWith<TransactionEnvelope> for IssueEventsFilter {
    fn id(&self) -> u32 {
        5 // any number that is unique from the other filters
    }

    fn check_for_processing(&self, param: &TransactionEnvelope) -> bool {
        if let TransactionEnvelope::EnvelopeTypeTx(tx_env) = param {
            match tx_env.tx.memo {
                // we will only process this transaction if the issue_id is in the list.
                Memo::MemoHash(hash) => {  return self.issue_ids.contains(&hash); }
                _ => {}
            }
        }
        false
    }
}
```
Then with the `ScpMessageHandler`, just call:
```rust
handler.add_filter(Box::new(issue_events_filter)).await?;
```
The _`Box`_ is required because we want it to be flexible enough to accommodate different filters.

## Example
Run this command:
```
 RUST_LOG=info cargo run --example oracle
```
and you should be able to see in the terminal something similar when running the _`connect`_ example of `Stellar-Relay`.
But every 8 seconds, it logs the slots in the map:
```
[2022-10-19T08:34:03Z INFO  vault::oracle::collector::tx_handler] Inserting received transaction set for slot 574880
[2022-10-19T08:34:08Z INFO  vault::oracle::collector::envelopes_handler] Adding received SCP envelopes for slot 574881
[2022-10-19T08:34:08Z INFO  vault::oracle::collector::tx_handler] Inserting received transaction set for slot 574881
[2022-10-19T08:34:10Z INFO  oracle] Slots in the map: 14
[2022-10-19T08:34:13Z INFO  vault::oracle::collector::envelopes_handler] Adding received SCP envelopes for slot 574882
[2022-10-19T08:34:13Z INFO  vault::oracle::collector::tx_handler] Inserting received transaction set for slot 574882
[2022-10-19T08:34:18Z INFO  oracle] Slots in the map: 15
```