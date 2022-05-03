# Building

From the spacewalk/client directory run

```
cargo build --features=standalone-metadata
```

## Running the vault

```
cargo run --bin vault --features standalone-metadata  -- --keyring alice --stellar-vault-secret-key SB6WHKIU2HGVBRNKNOEOQUY4GFC4ZLG5XPGWLEAHTIZXBXXYACC76VSQ
```

## Tests

To run the tests (unit and integration tests) for the spacewalk vault client run

```
cargo test --package vault --features standalone-metadata
```

To run only the unit tests use

```
cargo test --package vault --lib --features standalone-metadata -- --nocapture
```

**Note** that when running the integration test the console might show errors like
```
ERROR vault::redeem: Error while sending request: error sending request for url (https://horizon-testnet.stellar.org/accounts/GA6ZDMRVBTHIISPVD7ZRCVX6TWDXBOH2TE5FAADJXZ52YL4GCFI4HOHU): error trying to connect: dns error: cancelled
```
but this does not mean that the test fails. 
The `test_redeem` integration test only checks if a `RedeemEvent` was emitted and terminates afterwards. 
This stops the on-going withdrawal execution the vault client started leading to that error. 
The withdrawal execution is tested in the `test_execute_withdrawal` unit test instead.


## Updating the metadata

```
cargo install subxt-cli

subxt metadata -f bytes > runtime/metadata-standalone.scale
```

## Troubleshooting

### Invalid spec version

If there are errors with spec versions not matching you might have to change the `DEFAULT_SPEC_VERSION` in runtime/src/rpc.rs.

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

If the transaction submission fails giving a `tx_failed` in the `result_codes` object of the response, this is likely due to the converted destination account not having trustlines set up for the redeemed asset.
The destination account is derived automatically from the account that called the extrinsic on-chain.
