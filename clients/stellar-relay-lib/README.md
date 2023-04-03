# Stellar Relay

A rust implementation of the [js-stellar-node-connector](https://github.com/stellarbeat/js-stellar-node-connector).

The Stellar Relay acts as a mediator between the user(you) and the Stellar Node.

### The `StellarOverlayConfig`
```rust
pub struct StellarOverlayConfig { 
    stellar_history_base_url: String, 
    connection_info: ConnectionInfoCfg, 
    node_info: NodeInfoCfg,
}
```
The `StellarOverlayConfig` is a configuration to connect to the Stellar Node. It contains the following:
 * `stellar history base url` - to access the archive
 * `ConnectionInfoCfg`
 * `NodeInfoCfg`.

The `NodeInfoCfg` contains the information of the Stellar Node to connect to. Except the address and the port.
```rust
pub struct NodeInfoCfg {
    pub ledger_version: u32,
    pub overlay_version: u32,
    pub overlay_min_version: u32,
    pub version_str: Vec<u8>,
    pub is_pub_net: bool,
}
```
Check out [Stellarbeat.io](https://stellarbeat.io/) for examples.

The `ConnectionInfoCfg` is a configuration for connecting to the Stellar Node. It is here where we specify the address and port.
```rust
pub struct ConnectionInfoCfg {
    /// Stellar Node Address
    address: String,
    /// Stellar Node port
    port: u32,
    pub auth_cert_expiration: u64,
    pub recv_tx_msgs: bool,
    pub recv_scp_msgs: bool,
    pub remote_called_us: bool,
    /// how long to wait for the Stellar Node's messages.
    timeout_in_secs: u64,
    /// number of retries to wait for the Stellar Node's messages and/or to connect back to it.
    retries:u8
}
```

## Usage

### Provide the `StellarOverlayConfig` file path

Start with the creating a **json** config file (see [here](resources) for example files).
The config file will be converted to a `StellarOverlayConfig`. using the function:
```rust 
let cfg = StellarOverlayConfig::try_from_path(<your_file_path>)?;
```

### Create the `StellarOverlayConnection`
Two things are needed to create a connection:
* **_secret key_**
* And given the `StellarOverlayConfig`  

Create a connection using the `connect_to_stellar_overlay_network` function:
```rust
let mut overlay_connection = stellar_relay_lib::connect_to_stellar_overlay_network(cfg, secret_key).await?;
```
The `StellarOverlayConnection` has 2 async methods to interact with the Stellar Node:
* _`send(&self, message: StellarMessage)`_ -> for sending `StellarMessage`s to Stellar Node
* _`listen(&mut self)`_ -> for receiving `StellarRelayMessage`s from the Stellar Relay.

### Interpreting the `StellarRelayMessage`
The `StellarRelayMessage` is an enum with the following variants:
* _`Connect`_ -> interprets a successful connection to Stellar Node. It contains the `PublicKey` and the `NodeInfo`
* _`Data`_ -> a wrapper of a `StellarMessage` and additional fields: the _message type_ and the unique `p_id`(process id) 
* _`Timeout`_ -> Depends on the `timeout_in_secs` and `retries` defined in the `ConnectionInfo` (**10** and **3** by default). This message is returned after multiple retries have been done.
For example, Stellar Relay will wait for 10 seconds to read from the existing tcp stream before retrying again. After the 3rd retry, StellarRelay will create a new stream in 3 attempts, with an interval of 3 seconds.
* _`Error`_ -> a todo

## Example
In the `stellar-relay` directory, run this command:
```
 RUST_LOG=info cargo run --example connect
```
and you should be able to see in the terminal:
```
[2022-10-14T13:16:00Z INFO  connect] Connected to "Test SDF Network ; September 2015" through "135.181.16.110"
[2022-10-14T13:16:00Z INFO  stellar_relay::connection::services] Starting Handshake with Hello.
[2022-10-14T13:16:01Z INFO  stellar_relay::connection::connector::message_handler] Hello message processed successfully
[2022-10-14T13:16:01Z INFO  stellar_relay::connection::connector::message_handler] Handshake completed
[2022-10-14T13:16:01Z INFO  connect] Connected to Stellar Node: "AAAAAAaweClXqq3sjNIHBm/r6o1RY6yR5HqkHJCaZtEEdMUf"
[2022-10-14T13:16:01Z INFO  connect] NodeInfo { ledger_version: 19, overlay_version: 24, overlay_min_version: 21, version_str: [118, 49, 57, 46, 52, 46, 48], network_id: [122, 195, 57, 151, 84, 78, 49, 117, 210, 102, 189, 2, 36, 57, 178, 44, 219, 22, 80, 140, 1, 22, 63, 38, 229, 203, 42, 62, 16, 69, 169, 121] }
[2022-10-14T13:16:01Z INFO  connect] rcv StellarMessage of type: Peers
[2022-10-14T13:16:01Z INFO  connect] rcv StellarMessage of type: GetScpState
[2022-10-14T13:16:02Z INFO  connect] R0JCUVFUM0VJVVNYUkpDNlRHVUNHVkEzRlZQWFZaTEdHM09KWUFDV0JFV1lCSFU0NldKTFdYRVU= sent StellarMessage of type ScpStNominate  for ledger 43109751
[2022-10-14T13:16:02Z INFO  connect] R0RYUUIzT01NUTZNR0c0M1BXRkJaV0JGS0JCRFVaSVZTVURBWlpUUkFXUVpLRVMyQ0RTRTVIS0o= sent StellarMessage of type ScpStNominate  for ledger 43109751
...
[2022-10-14T13:16:02Z INFO  connect] rcv StellarMessage of type: Transaction
[2022-10-14T13:16:02Z INFO  connect] rcv StellarMessage of type: Transaction
[2022-10-14T13:16:02Z INFO  connect] rcv StellarMessage of type: Transaction
...
[2022-10-14T13:16:02Z INFO  connect] R0E1U1RCTVY2UURYRkRHRDYyTUVITExIWlRQREk3N1UzUEZPRDJTRUxVNVJKREhRV0JSNU5OSzc= sent StellarMessage of type ScpStNominate  for ledger 43109751
[2022-10-14T13:16:02Z INFO  connect] R0NHQjJTMktHWUFSUFZJQTM3SFlaWFZSTTJZWlVFWEE2UzMzWlU1QlVEQzZUSFNCNjJMWlNUWUg= sent StellarMessage of type ScpStPrepare for ledger 43109751
```

Here is an example in the terminal when disconnection/reconnection happens:
```
[2022-10-17T05:56:47Z ERROR stellar_relay::connection::services] deadline has elapsed for reading messages from Stellar Node. Retry: 0
[2022-10-17T05:56:47Z ERROR stellar_relay::connection::services] deadline has elapsed for receiving messages. Retry: 0
[2022-10-17T05:56:57Z ERROR stellar_relay::connection::services] deadline has elapsed for reading messages from Stellar Node. Retry: 1
[2022-10-17T05:56:57Z ERROR stellar_relay::connection::services] deadline has elapsed for receiving messages. Retry: 1
[2022-10-17T05:57:07Z ERROR stellar_relay::connection::services] deadline has elapsed for reading messages from Stellar Node. Retry: 2
[2022-10-17T05:57:07Z ERROR stellar_relay::connection::services] deadline has elapsed for receiving messages. Retry: 2
[2022-10-17T05:57:17Z ERROR stellar_relay::connection::services] deadline has elapsed for reading messages from Stellar Node. Retry: 3
[2022-10-17T05:57:17Z ERROR stellar_relay::connection::services] deadline has elapsed for receiving messages. Retry: 3
[2022-10-17T05:57:17Z INFO  stellar_relay::connection::user_controls] reconnecting to "135.181.16.110".
[2022-10-17T05:57:17Z ERROR stellar_relay::connection::user_controls] failed to reconnect! # of retries left: 2. Retrying in 3 seconds...
[2022-10-17T05:57:20Z INFO  stellar_relay::connection::user_controls] reconnecting to "135.181.16.110".
[2022-10-17T05:57:20Z INFO  stellar_relay::connection::services] Starting Handshake with Hello.
[2022-10-17T05:57:21Z INFO  stellar_relay::connection::connector::message_handler] Hello message processed successfully
[2022-10-17T05:57:21Z INFO  stellar_relay::connection::connector::message_handler] Handshake completed
```


todo: add multiple tests