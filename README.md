# Spacewalk: a Stellar bridge

This repository contains the pallets needed to set up spacewalk as well as a standalone chain configured to run and test the spacewalk bridge.

# Overview

Spacewalk is a bridge between Substrate-based parachains and Stellar which enables asset transfers to and from Stellar. This grant application is for developing the Spacewalk protocol and pallet. The Spacewalk bridge is built by the team behind the Pendulum network (an upcoming parachain that connects fiat tokens from across multiple blockchain ecosystems).

## Issue

Issuing refers to transferring funds from Stellar to a Substrate-based chain implementing the spacewalk pallet.
It's a two-step process where a user sends assets to a "locking account" (held by a vault client), after which the spacewalk pallet then mints tokens on the Substrate chain.

First, a Stellar account submits a payment transaction sending assets to the Stellar account of a vault.
Every vault periodically checks for new transactions of its Stellar account. If it identifies a new transaction, it will report it to the spacewalk pallet of the substrate-based chain it is connected to.
The spacewalk pallet then validates the reported transaction and eventually credits the corresponding account on the Substrate chain.
The credited account is derived from the public key of the Stellar source account.

### Vault Client

Every vault client is supplied with the secret key of a Stellar account to monitor issuing transactions and redeeming funds.
The handling of issue requests is implemented in `clients/vault/src/deposit.rs`.
For a specific interval (currently every 5 seconds), the vault client fetches the latest Stellar transaction of its Stellar account.
If it identifies a new transaction, i.e., the ID of the latest fetched transaction differs from the one it saw last, it will report this transaction (encoded in XDR representation) to the spacewalk pallet.

### Spacewalk Pallet

The spacewalk pallet offers the `report_stellar_transaction(transaction_envelope_xdr)` extrinsic.
The extrinsic expects an encoded Stellar transaction XDR containing a payment operation as an argument.
This extrinsic can be called to notify the pallet that a transaction to a vault account has taken place, announcing that it should credit the funds contained in the payment operation to the corresponding account on Substrate chain.
The `report_stellar_transaction()` extrinsic is implemented in `pallets/spacewalk/src/lib.rs`.

At the moment, the extrinsic does not contain safety checks because, for now, we assume that this extrinsic is only used in a non-malicious way.

## Redeem

Redeeming refers to the process of transferring funds back from the Substrate chain to the Stellar network.

The redemption starts with the spacewalk pallet burning a specific amount of tokens a user wants to transfer back to Stellar, which are then released from the "locking account" (held by a vault client) back to the corresponding user account on Stellar.

### Spacewalk Pallet

The spacewalk pallet offers the `redeem(asset_code, asset_issuer, amount, stellar_vault_pubkey)` extrinsic.
The asset code and issuer specify the asset that is to be withdrawn. The `stellar_vault_pubkey` encodes the public key of the vault/locking account that holds the user's funds on Stellar, i.e., the account previously used for issuing the assets on the Substrate chain.
After checking if the user has sufficient funds, the spacewalk pallet will burn the specified tokens and emit a `RedeemEvent`.
The `RedeemEvent` contains details about the redeemed asset and the public keys of the Stellar user account, and locking account/vault responsible for releasing the funds on Stellar.
The `redeem()` extrinsic is implemented in `pallets/spacewalk/src/lib.rs`.

### Vault Client

The vault client listens to the `RedeemEvents` emitted by the spacewalk pallet.
Once it detects a new redeem has taken place, the vault compares its public key to the `stellar_vault_id` contained in the event to detect if it is expected to act on this event.
If it was, it creates and submits a payment transaction transferring the previously locked funds to the Stellar user specified in the redeem event.
The handling of redeem events is implemented in `clients/vault/src/redeem.rs`.

### Spacewalk Pallet

The spacewalk pallet offers the `redeem(asset_code, asset_issuer, amount, stellar_vault_pubkey)` extrinsic.
The asset code and issuer specify the asset that is to be withdrawn, and the `stellar_vault_pubkey` is an encoding of the public key of the vault/locking account that holds the users funds on Stellar, i.e. that was previously used for issuing the assets on the Substrate chain.
After checking if the user has sufficient funds, the spacewalk pallet will burn the specified tokens and emit a `RedeemEvent`.
The `RedeemEvent` contains details about the redeemed asset, as well as the public keys of the Stellar user account and locking account/vault responsible for releasing the funds on Stellar.
The `redeem()` extrinsic is implemented in `pallets/spacewalk/src/lib.rs`.

### Vault Client

The vault client listens to the `RedeemEvents` emitted by the spacewalk pallet.
Once it detects a new redeem has taken place, the vault compares its public key to the `stellar_vault_id` that is contained in the event to detect if is expected to act on this event.
If it was, it creates and submits a payment transaction transferring the previously locked funds to the Stellar user specified in the redeem event.
The handling of redeem events is implemented in `clients/vault/src/redeem.rs`.

# Build and Run

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
