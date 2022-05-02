# Spacewalk: a Stellar bridge

This repository contains the pallets needed to set up spacewalk as well as a standalone chain configured to run and test the spacewalk bridge.

## Overview

Spacewalk is a bridge between Substrate-based parachains and Stellar which enables asset transfers to and from Stellar. This grant application is for developing the Spacewalk protocol and pallet. The Spacewalk bridge is built by the team behind the Pendulum network (an upcoming parachain that connects fiat tokens from across multiple blockchain ecosystems).

## Run all tests

```
cargo test
```

## Compile and run the testchain

```
cargo run --release -- --dev
```

## Run testchain without recompiling

```
./target/release/node-template --dev
```