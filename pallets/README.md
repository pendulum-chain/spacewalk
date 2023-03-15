# Spacewalk bridge pallets

Here you will find the different pallets needed to set up the spacewalk bridge.

## Testing

To run the tests use:

```bash
cargo test --package <pallet-name> --features runtime-benchmarks
```


## Benchmarking

### Running manually

Build the node with the `runtime-benchmarks` feature:

```bash
cargo build --package spacewalk-standalone --release --features runtime-benchmarks
```

```bash
# Show benchmarks for this pallet
./target/release/spacewalk-standalone benchmark pallet -p <pallet-name> -e '*' --list
```

Run the benchmarking for a pallet:

```bash
./target/release/spacewalk-standalone benchmark pallet \
--chain=dev \
--pallet=fee \
--extrinsic='*' \
--steps=100 \
--repeat=10 \
--wasm-execution=compiled \
--output=pallets/<pallet-name>/src/default_weights.rs \
--template=./.maintain/frame-weight-template.hbs
```

### Using benchmarking scripts
Run benchmark for a single pallet

```bash
 ./maintain/run-benchmark --pallet <pallet_name>
 ```

Run benchmark for all pallets under the `/pallets` folder

```bash
  ./maintain/run-benchmark --all
```

The output should be as follows if you already compiled the binary with `--features runtime-benchmarks`:

```
Binary found ✓
Feature benchmark present ✓
Running all...
Run for pallet currency...✗
Run for pallet fee...✓
Run for pallet issue...✓
Run for pallet nomination...✓
Run for pallet oracle...✓
Run for pallet redeem...✓
Run for pallet replace...✓
Run for pallet reward...✗
Run for pallet security...✗
Run for pallet staking...✗
Run for pallet stellar-relay...✓
Run for pallet vault-registry...✓
```

If you don't have the binary or the feature enabled, the script will re-compile the project for you, including the benchmarking feature; then it will run the desired benchmarks.

**Important note**: the output of the benchmarks is hidden for convinence. In most cases, finding a ✗ means that there are no benchmarks for that pallet, so you're safe. But, if you feel something is wrong and you want to find the reason of the errors for a single pallet, you can still run `./maintain/run-benchmark --pallet <pallet_name>`. The script will output the error, if any. For any other debug levels, you need to run the benchmarks manually.
