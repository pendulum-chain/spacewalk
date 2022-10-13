# Stellar Relay

A rust implementation of the [js-stellar-node-connector](https://github.com/stellarbeat/js-stellar-node-connector).

## Usage
### Provide the `NodeInfo` and `ConnConfig`
 The `NodeInfo` contains the information of the Stellar Node to connect to.
```rust
pub struct NodeInfo {
    pub ledger_version: u32,
    pub overlay_version: u32,
    pub overlay_min_version: u32,
    pub version_str: Vec<u8>,
    pub network_id: NetworkId,
}
```
Check out [Stellarbeat.io](https://stellarbeat.io/) for examples.

The `ConnConfig` is a configuration for connecting to the Stellar Node. It is here where we specify the address and port.
```rust
pub struct ConnConfig {
    /// Stellar Node Address
    address: String,
    /// Stellar Node port
    port: u32,
    secret_key: SecretKey,
    pub auth_cert_expiration: u64,
    pub recv_tx_msgs: bool,
    pub recv_scp_messages: bool,
    pub remote_called_us: bool,
    /// how long to wait for the Stellar Node's messages.
    timeout_in_secs: u64,
    /// number of retries to wait for the Stellar Node's messages and/or to connect back to it.
    retries:u8
}
```
