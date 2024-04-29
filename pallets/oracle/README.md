# Oracle

Based on the Oracle specification [https://spec.interlay.io/spec/oracle.html](https://spec.interlay.io/spec/oracle.html).

## Installation

Run `cargo build` from the root folder of this directory.

## Runtime Integration

### Runtime `Cargo.toml`

To add this pallet to your runtime, simply include the following to your runtime's `Cargo.toml` file:

```TOML
[dependencies.stellar-relay]
default_features = false
git = '../creates/oracle'
```

Update your runtime's `std` feature to include this pallet:

```TOML
std = [
    # --snip--
    'oracle/std',
]
```

### Runtime `lib.rs`

You should implement it's trait like so:

```rust
/// Used for test_module
impl oracle::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}
```

and include it in your `construct_runtime!` macro:

```rust
Oracle: oracle::{Module, Call, Config<T>, Storage, Event<T>},
```

## Reference Docs

You can view the reference docs for this pallet by running:

```
cargo doc --open
```


## Testing

To run the tests use:

```bash
cargo +nightly-2024-02-09 test --package oracle --features runtime-benchmarks
```

## Benchmarking

Build the node with the `runtime-benchmarks` feature:

```bash
cargo build --package spacewalk-standalone --release --features runtime-benchmarks
```

```bash
# Show benchmarks for this pallet
./target/release/spacewalk-standalone benchmark pallet -p oracle -e '*' --list
```

Run the benchmarking for a pallet:

```bash
./target/release/spacewalk-standalone benchmark pallet \
--chain=dev \
--pallet=oracle \
--extrinsic='*' \
--steps=100 \
--repeat=10 \
--wasm-execution=compiled \
--output=pallets/oracle/src/default_weights.rs \
--template=./.maintain/frame-weight-template.hbs
```