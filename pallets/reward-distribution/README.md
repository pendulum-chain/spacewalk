# Reward distribution pallet

This pallet contains the logic to distribute rewards to the Spacewalk vault clients and their nominators.

## Testing

To run the tests use:

```bash
cargo test --package reward-distribution --features runtime-benchmarks
```

## Benchmarking

Build the node with the `runtime-benchmarks` feature:

```bash
cargo build --package spacewalk-standalone --release --features runtime-benchmarks
```bash

```bash
# Show benchmarks for this pallet
./target/release/spacewalk-standalone benchmark pallet -p reward-distribution -e '*' --list
```bash

Run the benchmarking for a pallet:

```bash
./target/release/spacewalk-standalone benchmark pallet \
--chain=dev \
--pallet=reward-distribution \
--extrinsic='*' \
--steps=100 \
--repeat=10 \
--wasm-execution=compiled \
--output=pallets/reward-distribution/src/default_weights.rs \
--template=./.maintain/frame-weight-template.hbs
```bash

