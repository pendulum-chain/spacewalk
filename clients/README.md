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
cargo run --bin vault --features standalone-metadata  -- --keyring alice
```
