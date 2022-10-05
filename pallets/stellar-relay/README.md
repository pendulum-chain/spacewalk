# Stellar Relay Pallet

## Testing

To run the tests use:

```bash
cargo test --package pallet-stellar-relay --features runtime-benchmarks
```

## Benchmarking

Build the node with the `runtime-benchmarks` feature:

```bash
cargo build --package spacewalk-standalone --release --features runtime-benchmarks
```

```bash
# Show all available benchmarks
./target/release/spacewalk-standalone benchmark pallet --list

# Show benchmarks for stellar relay pallet
./target/release/spacewalk-standalone benchmark pallet -p pallet-stellar_relay -e '*' --list
```

Run the benchmarking for the `pallet-stellar_relay` pallet:

```bash
./target/release/spacewalk-standalone benchmark pallet \
--chain dev \
--pallet pallet_stellar_relay \
--extrinsic '*' \
--steps 20 \
--repeat 10 \
--output pallets/stellar-relay/src/weights.rs
```