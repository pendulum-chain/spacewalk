# Spacewalk bridge pallets

Here you will find the different pallets needed to set up the spacewalk bridge.

## Testing

To run the tests use:

```bash
cargo test --package <pallet-name> --features runtime-benchmarks
```

## Benchmarking

Build the node with the `runtime-benchmarks` feature:

```bash
cargo build --package spacewalk-standalone --release --features runtime-benchmarks
```

```bash
# Show all available benchmarks
./target/release/spacewalk-standalone benchmark pallet --list

# Show benchmarks for a specific pallet
./target/release/spacewalk-standalone benchmark pallet -p <pallet-name> -e '*' --list
```

Run the benchmarking for a pallet:

```bash
./target/release/spacewalk-standalone benchmark pallet \
--chain dev \
--pallet <pallet-name> \
--extrinsic '*' \
--steps 20 \
--repeat 10 \
--output pallets/<pallet-name>/src/weights.rs \
--template=./.maintain/frame-weight-template.hbs
```
