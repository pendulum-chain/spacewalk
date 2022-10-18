# Oracle

The Stellar Oracle uses [Stellar-Relay](../../../stellar-relay) to listen to messages from the StellarNode.  
The Oracle collects and saves **`SCPStatementExternalize`** SCPMessages and its corresponding `TransactionSets`.

## Usage

### Provide the `NodeInfo` and `ConnConfig` 
Refer to [Stellar-Relay readme](../../../stellar-relay/README.md) on how to initialize these structures.

### Create the `ScpMessageHandler`
Simply call the _async_ function `create_handler()`:
```rust
let handler = create_handler(node_info, cfg, is_public_network, vault_addresses_filter).await?;
```
The `vault_address_filter` is a list of addresses.
