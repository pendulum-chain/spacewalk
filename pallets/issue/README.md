# Issue pallet

## Testing

To run the tests use:

```bash
cargo +nightly test --package issue --features runtime-benchmarks
```

## Benchmarking

Build the node with the `runtime-benchmarks` feature:

```bash
cargo build --package spacewalk-standalone --release --features runtime-benchmarks
```

```bash
# Show benchmarks for this pallet
./target/release/spacewalk-standalone benchmark pallet -p issue -e '*' --list
```

Run the benchmarking for a pallet:

```bash
./target/release/spacewalk-standalone benchmark pallet \
--chain=dev \
--pallet=issue \
--extrinsic='*' \
--steps=100 \
--repeat=10 \
--wasm-execution=compiled \
--output=pallets/issue/src/default_weights.rs \
--template=./.maintain/frame-weight-template.hbs
```
