# Building

From the spacewalk/client directory run

```
// standalone version
cargo build --features=standalone-metadata

// parachain version
cargo build --features=parachain-metadata
```

## Running the vault

To run the vault with the auto-updater service:

```
cargo run --bin runner -- --parachain-ws <parachain-ws> -- --keyring alice  --stellar-vault-secret-key-filepath <secret_key_file_path> --stellar-overlay-config-filepath <cfg_file_path>```
```

To run the vault directly with the provided standalone chain use:

```
cargo run --bin vault --features standalone-metadata  -- --keyring alice --stellar-vault-secret-key-filepath <secret_key_file_path> --stellar-overlay-config-filepath <cfg_file_path>
```
 * Instead of flags,you can also use environment variables _`STELLAR_VAULT_SECRET_KEY_FILEPATH`_, _`STELLAR_OVERLAY_CONFIG_FILEPATH`_:
   ```
   export STELLAR_VAULT_SECRET_KEY_FILEPATH=<secret_key_file_path>
   export STELLAR_OVERLAY_CONFIG_FILEPATH=<cfg_file_path>
   ```
 * To see examples of the config file, check [here](stellar-relay-lib/resources/config). 
 * An example of the secret key file path is found [here](stellar-relay-lib/resources/secretkey).

### auto-register
To make the vault auto-register itself with the chain, there are 2 options:
* use the _`--auto-register`_ flag:
  * ```
    --auto-register 0,GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN:USDC,1000000
    ```
* use the environment variable _`AUTO_REGISTER`_. The value must be a tuple, so enclose it with double quotes:
  * ```
    export AUTO_REGISTER="0,GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC:USDC,1000000"
    ```
Be careful with the asset pair you use, as using arbitrary asset pairs will result in the vault not being able to
register itself.
This is because some thresholds for the vault are set based on the currencies specified in the runtime and these
thresholds are not set for any currency.
The auto-register feature takes a string argument with the syntax
of `<collateral-currency>,<wrapped-currency>,<collateral-amount>`.
The `collateral-currency` is the currency used as collateral for the vault and assumed to be an XCM currency, with the
number indicating its index for XCM that is configured per runtime.
The `wrapped-currency` can be either XCM or a Stellar asset, thus the format would be either just `XLM`
or `<wrapped-currency-issuer>:<wrapped-currency-code>`.

```
# Run the vault with auto-registering for XLM
cargo run --bin vault --features standalone-metadata  -- --keyring alice --stellar-vault-secret-key-filepath <secret_key_file_path> --stellar-overlay-config-filepath <cfg_file_path> --auto-register "0,XLM,1000000"

# Run the vault with auto-registering for the USDC asset on testnet (assuming GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC as the issuer)
cargo run --bin vault --features standalone-metadata  -- --keyring alice --stellar-vault-secret-key-filepath <secret_key_file_path> --stellar-overlay-config-filepath <cfg_file_path> --auto-register "0,GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC:USDC,1000000"

# Run the vault with auto-registering for the USDC asset on mainnet (assuming the issuer is centre.io)
cargo run --bin vault --features standalone-metadata  -- --keyring alice --stellar-vault-secret-key-filepath <secret_key_file_path> --stellar-overlay-config-filepath <cfg_file_path> --auto-register "0,GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN:USDC,1000000"
```

### specifying parachains
To run the vault with a parachain (e.g. Pendulum) you need to specify the URL using 2 options:
* use the _`spacewalk-parachain-url`_ flag:
  * ```
    --spacewalk-parachain-url ws://localhost:8844
    ```
* use the environment variable _`SPACEWALK_PARACHAIN_URL`_:
  * ```
    export SPACEWALK_PARACHAIN_URL=ws://localhost:8844
    ```

## Tests

### Prerequisites

For the successful completion of some tests, you need to have access to Stellar accounts.
You need to have a source and destination account for each Stellar mainnet and testnet.
The secrets for these accounts can either be provided as environment variables or in a file.
The file should contain the secrets in the following format and placed in `/clients/.env`:

```
SOURCE_STELLAR_SECRET_MAINNET=SABC...
DEST_STELLAR_SECRET_MAINNET=SABC...

SOURCE_STELLAR_SECRET_TESTNET=SABC...
DEST_STELLAR_SECRET_TESTNET=SABC...
```

You can also find an example for setting these variables in the `.env.example` file.

### Running the tests

**Note** Tests should run using Rust **_`nightly`_** version. Make sure to install and add the target `wasm32-unknown-unknown`.

To run the tests (unit and integration tests) for the spacewalk vault client run

```
cargo +nightly test --package vault --all-features
```

To run only the unit tests use

```
cargo +nightly test --package vault --lib --bins --features standalone-metadata
```

To run only the integration tests use

```
cargo +nightly test --test '*' --package vault --features integration-test
```

**Note** that when running the integration test the console might show errors like

* ```
  ERROR vault::redeem: Error while sending request: error sending request for url (https://horizon-testnet.stellar.org/accounts/GA6ZDMRVBTHIISPVD7ZRCVX6TWDXBOH2TE5FAADJXZ52YL4GCFI4HOHU): error trying to connect: dns error: cancelled
  ```
    * but this does not mean that the test fails.
      The `test_redeem` integration test only checks if a `RedeemEvent` was emitted and terminates afterwards.
      This stops the on-going withdrawal execution the vault client started leading to that error.
      The withdrawal execution is tested in the `test_execute_withdrawal` unit test instead.

* ```
    ERROR offchain-worker::http: Requested started id=1630 method=POST uri=https://dia-00.pendulumchain.tech:8070/currencies
    ERROR dia_oracle::pallet: Failed to Update Prices HttpRequestFailed
  ```
    * these errors can be ignored

## Updating the metadata

When any changes are made to elements associated with the pallets, such as extrinsic names or parameters, it is
necessary to regenerate the metadata. Subxt is employed specifically for this purpose.
Note: The `--version 14` flag is required when connecting to runtime nodes that are running older Substrate
dependencies.
This is the case for the runtimes using polkadot dependencies below version v1.0.0.

### Installing subxt

To install subxt, run the following command:

```
cargo install subxt-cli@0.31.0
```

You might encounter issues with the installation of subxt, because it needs a more up-to-date version of the rust
toolchain.
To bypass this issue, you can change the version of the rust toolchain in the `rust-toolchain` file to the latest
version and try installing subxt again.
You need to change the version back however, in order to avoid issues with other dependencies being built.

### Updating the standalone-metadata file

After making the changes, the first step is to run the local node.

```
cargo run --bin spacewalk-standalone --release -- --dev
```

Once the local node is running, run this command from the clients directory:

```
subxt metadata -f bytes > runtime/metadata-standalone.scale --version 14
```

### Updating the metadata of the supported runtimes (Pendulum/Amplitude/Foucoco)

To update the metadata of the supported parachains, run the following commands:

```
subxt metadata -f bytes --url wss://rpc-pendulum.prd.pendulumchain.tech:443 > runtime/metadata-parachain-pendulum.scale --version 14
subxt metadata -f bytes --url wss://rpc-amplitude.pendulumchain.tech:443 > runtime/metadata-parachain-amplitude.scale --version 14
subxt metadata -f bytes --url wss://rpc-foucoco.pendulumchain.tech:443 > runtime/metadata-parachain-foucoco.scale --version 14
```

To facilitate comparison of changes in metadata, it is recommended to also convert the metadata to JSON format. This can
be done using the following commands:

```
subxt metadata -f json --url wss://rpc-pendulum.prd.pendulumchain.tech:443 > runtime/metadata-parachain-pendulum.json --version 14
subxt metadata -f json --url wss://rpc-amplitude.pendulumchain.tech:443 > runtime/metadata-parachain-amplitude.json --version 14
subxt metadata -f json --url wss://rpc-foucoco.pendulumchain.tech:443 > runtime/metadata-parachain-foucoco.json --version 14
```

## Troubleshooting

### Invalid spec version

If there are errors with spec versions not matching you might have to change the `DEFAULT_SPEC_VERSION` in
runtime/src/rpc.rs.

### Building on macOS

If you are encountering build errors on macOS try the following steps:

1. Install llvm with brew (`brew install llvm`).

1. Install wasm-pack for cargo (`cargo install wasm-pack`).

1. Set clang variables

```
# on intel CPU
export AR=/usr/local/opt/llvm/bin/llvm-ar
export CC=/usr/local/opt/llvm/bin/clang
# on M1 CPU
export AR=/opt/homebrew/opt/llvm/bin/llvm-ar
export CC=/opt/homebrew/opt/llvm/bin/clang
```

### Transaction submission failed

If the transaction submission fails giving a `tx_failed` in the `result_codes` object of the response, this is likely
due to the converted destination account not having trustlines set up for the redeemed asset.
The destination account is derived automatically from the account that called the extrinsic on-chain.

### Debugging with `tokio-console`

The vault is `tokio-console` ready. [Install tokio-console](https://github.com/tokio-rs/console?tab=readme-ov-file#running-the-console)
and connect to the vault.  
If using the [testchain](../testchain) and vault's `standalone-metadata` feature, you can use the command:
```
tokio-console http://127.0.0.1:6669
```
This will display: 
<img width="1138" alt="Screenshot 2024-05-13 at 6 33 33 PM" src="https://github.com/pendulum-chain/spacewalk/assets/2826165/6681a16c-84c0-47f5-abc6-e4ba6a7dc032">
The multiple ` tokio::task clients/vault/src/system.rs ` tasks follows the tasks spawned consecutively, in [system.rs](https://github.com/pendulum-chain/spacewalk/blob/main/clients/vault/src/system.rs):  

* The first 4 tasks are from `fn create_initial_tasks(...)` :
    * VaultId Registration Listener
    * Restart Timer
    * Stellar Transaction Listener
    * Parachain Block Listener
* Next 5 tasks from `fn create_issue_tasks(...)` :  
    * Issue Request Listener
    * Issue Cancel Listener
    * Issue Execute Listener
    * Issue Executor
    * Issue Cancel Scheduler
* Next 4 tasks from `fn create_replace_tasks(...)` :  
    * Request Replace Listener
    * Accept Replace Listener
    * Execute Replace Listener
    * Replace Cancellation Scheduler   
* Redeem Request Listener
* The last 2 tasks from `create_bridge_metrics_tasks(...)` :  
    * Bridge Metrics Listener
    * Bridge Metrics Poller


## Notes on the implementation of subxt

This section is supposed to help when encountering issues with communication of vault client and parachain.

### The runtime configuration

In `runtime/src/lib.rs` the `Config` for the runtime is defined.
These are the types that are used by subxt when connecting to the target chain.
Note that, although the `Config` types are all declared explicitly here, it would also work to use
the `subxt::PolkadotConfig` type.
This type is defined in the `subxt` crate and contains all the types that are used in the Polkadot runtime.
Using the `subxt::SubstrateConfig` type does not work however, because the `ExtrinsicParams` type does not work with the
testchain.
When encountering an error with 'validate_transaction() failed' it is likely that the `Config` type is not set
correctly.

### Trait derivations

It might happen that you encounter errors complaining about missing trait derivations.
There are different ways to derive traits for the automatically generated types.
You can either implement the traits manually (see the modules in `runtime/src/rpc.rs`) or use the respective
statements in the `#[subxt::subxt]` macro.
More documentation can be
found [here](https://docs.rs/subxt-macro/latest/subxt_macro/#adding-derives-for-specific-types).

```
#[subxt::subxt(
    runtime_metadata_path = "polkadot_metadata.scale",
    derive_for_all_types = "Eq, PartialEq",
    derive_for_type(type = "frame_support::PalletId", derive = "Ord, PartialOrd"),
    derive_for_type(type = "sp_runtime::ModuleError", derive = "Hash"),
)]
```

### Type substitutions

When the compiler complains about mismatched types although the types seem to be the same, you might have to use type
substitutions.
This is done by adding the `#[subxt(substitute_type = "some type")]` attribute to the metadata module.
More documentation can be found [here](https://docs.rs/subxt-macro/latest/subxt_macro/#substituting-types).
