# Client info pallet
This pallet contains storage items regarding the vault client info, and 2 extrinsics to set this value.

## Testing

To run the tests use:

```bash
cargo +nightly test --package clients-info --features runtime-benchmarks
```


## Benchmarking

Build the node with the `runtime-benchmarks` feature:

```bash
cargo build --package spacewalk-standalone --release --features runtime-benchmarks
```bash

```bash
# Show benchmarks for this pallet
./target/release/spacewalk-standalone benchmark pallet -p clients-info -e '*' --list
```bash

Run the benchmarking for a pallet:

```bash
./target/release/spacewalk-standalone benchmark pallet \
--chain=dev \
--pallet=clients-info \
--extrinsic='*' \
--steps=100 \
--repeat=10 \
--wasm-execution=compiled \
--output=pallets/clients-info/src/default_weights.rs \
--template=./.maintain/frame-weight-template.hbs
```bash

