# Building

Set clang variables

```
export AR=/usr/local/opt/llvm/bin/llvm-ar
export CC=/usr/local/opt/llvm/bin/clang
```

From the spacewalk/client directory run

```
cargo build --features=standalone-metadata
```

## Running the vault

```
cargo run --bin vault --features standalone-metadata  -- --keyring alice --stellar-escrow-secret-key SA4OOLVVZV2W7XAKFXUEKLMQ6Y2W5JBENHO5LP6W6BCPBU3WUZ5EBT7K
```

## Tests

To run the tests for the spacewalk client run

```
cargo test --package vault --features standalone-metadata
```

to run only the unit tests use

```
cargo test --package vault --lib --features standalone-metadata -- --nocapture
```

## Updating the metadata

```
cargo install subxt-cli

subxt metadata -f bytes > runtime/metadata-standalone.scale
```

## Troubleshooting

If there are errors with spec versions not matching you might have to change the `DEFAULT_SPEC_VERSION` in runtime/src/rpc.rs.
