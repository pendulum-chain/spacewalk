# Spacewalk: a Stellar bridge

This repository contains the pallets needed to set up spacewalk as well as a standalone chain configured to run and test
the spacewalk bridge.

# Overview

Spacewalk is a bridge between Substrate-based parachains and Stellar which enables asset transfers to and from Stellar.
This grant application is for developing the Spacewalk protocol and pallet. The Spacewalk bridge is built by the team
behind the Pendulum network (an upcoming parachain that connects fiat tokens from across multiple blockchain ecosystems)
.

## Issue

Issuing refers to transferring funds from Stellar to a Substrate-based chain implementing the spacewalk pallet.
It's a two-step process where a user sends assets to a "locking account" (held by a vault client), after which the
spacewalk pallet then mints tokens on the Substrate chain.

First, a Stellar account submits a payment transaction sending assets to the Stellar account of a vault.
Every vault periodically checks for new transactions of its Stellar account. If it identifies a new transaction, it will
report it to the spacewalk pallet of the substrate-based chain it is connected to.
The spacewalk pallet then validates the reported transaction and eventually credits the corresponding account on the
Substrate chain.
The credited account is derived from the public key of the Stellar source account.

### Vault Client

Every vault client is supplied with the secret key of a Stellar account to monitor issuing transactions and redeeming
funds.
The handling of issue requests is implemented in `clients/vault/src/deposit.rs`.
For a specific interval (currently every 5 seconds), the vault client fetches the latest Stellar transaction of its
Stellar account.
If it identifies a new transaction, i.e., the ID of the latest fetched transaction differs from the one it saw last, it
will report this transaction (encoded in XDR representation) to the spacewalk pallet.

### Spacewalk Pallet

The spacewalk pallet offers the `report_stellar_transaction(transaction_envelope_xdr)` extrinsic.
The extrinsic expects an encoded Stellar transaction XDR containing a payment operation as an argument.
This extrinsic can be called to notify the pallet that a transaction to a vault account has taken place, announcing that
it should credit the funds contained in the payment operation to the corresponding account on Substrate chain.
The `report_stellar_transaction()` extrinsic is implemented in `pallets/spacewalk/src/lib.rs`.

At the moment, the extrinsic does not contain safety checks because, for now, we assume that this extrinsic is only used
in a non-malicious way.

## Redeem

Redeeming refers to the process of transferring funds back from the Substrate chain to the Stellar network.

The redemption starts with the spacewalk pallet burning a specific amount of tokens a user wants to transfer back to
Stellar, which are then released from the "locking account" (held by a vault client) back to the corresponding user
account on Stellar.

### Spacewalk Pallet

The spacewalk pallet offers the `redeem(asset_code, asset_issuer, amount, stellar_vault_pubkey)` extrinsic.
The asset code and issuer specify the asset that is to be withdrawn. The `stellar_vault_pubkey` encodes the public key
of the vault/locking account that holds the user's funds on Stellar, i.e., the account previously used for issuing the
assets on the Substrate chain.
After checking if the user has sufficient funds, the spacewalk pallet will burn the specified tokens and emit
a `RedeemEvent`.
The `RedeemEvent` contains details about the redeemed asset and the public keys of the Stellar user account, and locking
account/vault responsible for releasing the funds on Stellar.
The `redeem()` extrinsic is implemented in `pallets/spacewalk/src/lib.rs`.

### Vault Client

The vault client listens to the `RedeemEvents` emitted by the spacewalk pallet.
Once it detects a new redeem has taken place, the vault compares its public key to the `stellar_vault_id` contained in
the event to detect if it is expected to act on this event.
If it was, it creates and submits a payment transaction transferring the previously locked funds to the Stellar user
specified in the redeem event.
The handling of redeem events is implemented in `clients/vault/src/redeem.rs`.

# Build and Run

There was
a [plan to replace Rust `nightly` version with the stable version](https://github.com/pendulum-chain/spacewalk/issues/506).   
The [default toolchain](./rust-toolchain.toml) is set to stable.  
However, the [pallet `currency`](./pallets/currency)'s
feature _`testing-utils`_ is using [**`mocktopus`**](https://docs.rs/mocktopus/latest/mocktopus/#) — a `nightly` only
lib — and will potentially break your IDEs:

```
error[E0554]: `#![feature]` may not be used on the stable release channel
 --> /.../registry/src/index.crates.io-6f17d22bba15001f/mocktopus_macros-0.7.11/src/lib.rs:6:12
  |
6 | #![feature(proc_macro_diagnostic)]
  |            ^^^^^^^^^^^^^^^^^^^^^
```

Building and testing is _different_. The `testing-utils` feature is for testing _**only**_, and **_requires_** *
*_`nightly`_**.

## Run all tests

To run the tests, use the Rust **nightly** version; minimum is `nightly-2024-02-09`.  
This allows [`mocktopus`](https://docs.rs/mocktopus/latest/mocktopus/#) to be used freely across all packages during
testing.

```
cargo +nightly test --lib --features standalone-metadata -- --nocapture
```

## Compile and run the testchain

```
cargo run --bin spacewalk-standalone --release -- --dev
```

## Run testchain without recompiling

```
./target/release/node-template --dev
```

## [`cmd-all` script](./scripts/cmd-all)

The `--all`, `--all-features` and `--all-target` flags _cannot be used_,
as [mentioned previously about the `currency` pallet's `testing-utils` feature](#Build-and-Run).    
The following commands (executed at the root directory) will **FAIL**:

* `cargo build --all-features `
* `cargo clippy --all-targets`

This "apply to all" script is necessary to execute a command across _all_ packages _**individually**_, adding the
required conditions to some.  
[Check the script on how it looks like](./scripts/cmd-all).

**_note_**: This has been tested with only 2 commands in the [CI workflow](.github/workflows/ci-main.yml): `check`
and `clippy`. Other commands might not work.

# Development

To ease the maintenance of proper code formatting and good practices, please consider installing the pre-commit hook
and/or the pre-push hook contained in this
repository.

The pre-commit hook runs `rustfmt` on all the changed files in a commit, to ensure that changed files are properly
formatted
before committing.
You can install the hook by running the following commands.

```
.maintain/add-precommit-hook.sh
```

The pre-push hool runs clippy checks that are also performed in the CI of the repository. These checks need to be
successful so the push actually happens.
Otherwise, pelase run the corresponding clippy fix command or manually fix the issue.

To install the hook, run:

```
.maintain/add-prepush-hook.sh
```

To ignore the checks once the hook has been installed, run `git push --no-verify`.

### Note

You may need to make the hook script executable. Pleas
run `chmod u+x .git/hooks/pre-push`,  `chmod u+x .git/hooks/pre-commit` if necessary.

# Releases

To automatically create a new release for Spacewalk, please follow these steps:

- Create a new release branch with the following command:
  ```
  git checkout -b release/vX.Y.Z
  ```
- Update the version number of all crates in `Cargo.toml` files. To do this you should use either
  the `.maintain/create_minor_release.sh` or `.maintain/create_patch_release.sh` script. The script will bump the
  version number of all crates.
- Create a new release commit with the following command:
  ```
    git commit -a -m "release: Release vX.Y.Z"
  ```
- Create a pull request for the release branch and merge it into the `main` branch. The title of this PR **has to
  contain**
  the **"release:"** prefix. This indicates that the CI should create a new release.

Once merged, the CI will create a new release and automatically publish it to GitHub.