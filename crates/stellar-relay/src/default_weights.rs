//! Autogenerated weights for stellar_relay
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2021-09-07, STEPS: `100`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 128

// Executed Command:
// target/release/interbtc-standalone
// benchmark
// --chain
// dev
// --execution=wasm
// --wasm-execution=compiled
// --pallet
// stellar-relay
// --extrinsic
// *
// --steps
// 100
// --repeat
// 10
// --output
// crates/stellar-relay/src/default_weights.rs
// --template
// .deploy/weight-template.hbs


#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for stellar_relay.
pub trait WeightInfo {
	fn verify_and_validate_transaction() -> Weight;
	fn verify_transaction_inclusion() -> Weight;
	fn validate_transaction() -> Weight;
}

/// Weights for stellar_relay using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: BTCRelay DisableInclusionCheck (r:1 w:0)
	// Storage: BTCRelay BestBlockHeight (r:1 w:0)
	// Storage: BTCRelay Chains (r:1 w:0)
	// Storage: BTCRelay BlockHeaders (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: BTCRelay StableParachainConfirmations (r:1 w:0)
	fn verify_and_validate_transaction() -> Weight {
		(66_727_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(7 as Weight))
	}
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: BTCRelay DisableInclusionCheck (r:1 w:0)
	// Storage: BTCRelay BestBlockHeight (r:1 w:0)
	// Storage: BTCRelay Chains (r:1 w:0)
	// Storage: BTCRelay BlockHeaders (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: BTCRelay StableParachainConfirmations (r:1 w:0)
	fn verify_transaction_inclusion() -> Weight {
		(38_910_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(7 as Weight))
	}
	// Storage: Security ParachainStatus (r:1 w:0)
	fn validate_transaction() -> Weight {
		(11_660_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: BTCRelay DisableInclusionCheck (r:1 w:0)
	// Storage: BTCRelay BestBlockHeight (r:1 w:0)
	// Storage: BTCRelay Chains (r:1 w:0)
	// Storage: BTCRelay BlockHeaders (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: BTCRelay StableParachainConfirmations (r:1 w:0)
	fn verify_and_validate_transaction() -> Weight {
		(66_727_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(7 as Weight))
	}
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: BTCRelay DisableInclusionCheck (r:1 w:0)
	// Storage: BTCRelay BestBlockHeight (r:1 w:0)
	// Storage: BTCRelay Chains (r:1 w:0)
	// Storage: BTCRelay BlockHeaders (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: BTCRelay StableParachainConfirmations (r:1 w:0)
	fn verify_transaction_inclusion() -> Weight {
		(38_910_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(7 as Weight))
	}
	// Storage: Security ParachainStatus (r:1 w:0)
	fn validate_transaction() -> Weight {
		(11_660_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
	}
}

