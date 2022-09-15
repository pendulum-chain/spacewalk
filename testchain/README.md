# Spacewalk Testchain

Based on the Standalone chain based on the Substrate Node Template
A fresh FRAME-based [Substrate](https://www.substrate.io/) node, ready for hacking :rocket:

### Rust Setup

First, complete the [basic Rust setup instructions](./docs/rust-setup.md).

### Run

Use Rust's native `cargo` command to build and launch the template node:

```sh
cargo run --release -- --dev
```

### Build

The `cargo run` command will perform an initial build. Use the following command to build the node
without launching it:

```sh
cargo build --release
```

### Embedded Docs

Once the project has been built, the following command can be used to explore all parameters and
subcommands:

```sh
./target/release/node-template -h
```

## Run

The provided `cargo run` command will launch a temporary node and its state will be discarded after
you terminate the process. After the project has been built, there are other ways to launch the
node.

### Single-Node Development Chain

This command will start the single-node development chain with non-persistent state:

```bash
./target/release/node-template --dev
```

Purge the development chain's state:

```bash
./target/release/node-template purge-chain --dev
```

Start the development chain with detailed logging:

```bash
RUST_BACKTRACE=1 ./target/release/node-template -ldebug --dev
```

> Development chain means that the state of our chain will be in a tmp folder while the nodes are
> running. Also, **alice** account will be authority and sudo account as declared in the
> [genesis state](https://github.com/substrate-developer-hub/substrate-node-template/blob/main/node/src/chain_spec.rs#L49)
> .
> At the same time the following accounts will be pre-funded:
> - Alice
> - Bob
> - Alice//stash
> - Bob//stash

## Troubleshooting

If you encounter an error similar to this when compiling the test chain on an Apple Silicon (M1) Mac:

```
thread 'main' panicked at 'Unable to find libclang: "the `libclang` shared library at /opt/homebrew/Cellar/llvm/14.0.6_1/lib/libclang.dylib could not be opened: dlopen(/opt/homebrew/Cellar/llvm/14.0.6_1/lib/libclang.dylib, 0x0005): tried: '/opt/homebrew/Cellar/llvm/14.0.6_1/lib/libclang.dylib' (mach-o file, but is an incompatible architecture (have (arm64), need (x86_64)))"' 
```

your best bet is to re-install the `llvm` package via `brew` by following [these](https://stackoverflow.com/a/71925201)
steps.
You have to install the `llvm` package via `brew` but specifying the `arch --x86_64` flag.
