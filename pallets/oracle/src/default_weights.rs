
//! Autogenerated weights for oracle
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-01-26, STEPS: `100`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `MacBook-Pro`, CPU: `<UNKNOWN>`
//! EXECUTION: None, WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/spacewalk-standalone
// benchmark
// pallet
// --chain=dev
// --pallet=oracle
// --extrinsic=*
// --steps=100
// --repeat=10
// --wasm-execution=compiled
// --output=pallets/oracle/src/default_weights.rs
// --template=.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for oracle.
pub trait WeightInfo {
	fn on_initialize() -> Weight;
	fn update_oracle_keys() -> Weight;
}

/// Weights for oracle using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: Timestamp Now (r:0 w:1)
	// Storage: Timestamp DidUpdate (r:0 w:1)
	fn on_initialize() -> Weight {
		// Minimum execution time: 0 nanoseconds.
		Weight::from_ref_time(0 as u64)
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Oracle OracleKeys (r:0 w:1)
	fn update_oracle_keys() -> Weight {
		// Minimum execution time: 8_000 nanoseconds.
		Weight::from_ref_time(9_000_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: Timestamp Now (r:0 w:1)
	// Storage: Timestamp DidUpdate (r:0 w:1)
	fn on_initialize() -> Weight {
		// Minimum execution time: 0 nanoseconds.
		Weight::from_ref_time(0 as u64)
			.saturating_add(RocksDbWeight::get().writes(2 as u64))
	}
	// Storage: Oracle OracleKeys (r:0 w:1)
	fn update_oracle_keys() -> Weight {
		// Minimum execution time: 8_000 nanoseconds.
		Weight::from_ref_time(9_000_000 as u64)
			.saturating_add(RocksDbWeight::get().writes(1 as u64))
	}
}