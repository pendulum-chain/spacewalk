# Building


From the spacewalk/client directory run

```
cargo build --features=standalone-metadata
```

## Running the vault

```
cargo run --bin vault --features standalone-metadata  -- --keyring alice --stellar-escrow-secret-key SB6WHKIU2HGVBRNKNOEOQUY4GFC4ZLG5XPGWLEAHTIZXBXXYACC76VSQ
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
