# Stellar Relay Pallet

## Testing

To run the tests use:

```bash
cargo +nightly-2024-02-09 test --package stellar-relay --features runtime-benchmarks
```

## Benchmarking

Build the node with the `runtime-benchmarks` feature:

```bash
cargo build --package spacewalk-standalone --release --features runtime-benchmarks
```

```bash
# Show benchmarks for this pallet
./target/release/spacewalk-standalone benchmark pallet -p stellar-relay -e '*' --list
```

Run the benchmarking for a pallet:

```bash
./target/release/spacewalk-standalone benchmark pallet \
--chain=dev \
--pallet=stellar-relay \
--extrinsic='*' \
--steps=100 \
--repeat=10 \
--wasm-execution=compiled \
--execution=wasm \
--output=pallets/stellar-relay/src/default_weights.rs \
--template=./.maintain/frame-weight-template.hbs
```
