
//! Autogenerated weights for stellar_relay
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-11-18, STEPS: `100`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `MacBook-Pro`, CPU: `<UNKNOWN>`
//! EXECUTION: None, WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/spacewalk-standalone
// benchmark
// pallet
// --chain=dev
// --pallet=stellar-relay
// --extrinsic=*
// --steps=100
// --repeat=10
// --wasm-execution=compiled
// --output=pallets/stellar-relay/src/default_weights.rs
// --template=.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for stellar_relay.
pub trait WeightInfo {
	fn update_tier_1_validator_set() -> Weight;
}

/// Weights for stellar_relay using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: StellarRelay Validators (r:0 w:1)
	// Storage: StellarRelay Organizations (r:0 w:1)
	fn update_tier_1_validator_set() -> Weight {
		// Minimum execution time: 23_000 nanoseconds.
		Weight::from_ref_time(23_000_000_u64)
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: StellarRelay Validators (r:0 w:1)
	// Storage: StellarRelay Organizations (r:0 w:1)
	fn update_tier_1_validator_set() -> Weight {
		// Minimum execution time: 23_000 nanoseconds.
		Weight::from_ref_time(23_000_000_u64)
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
}