
//! Autogenerated weights for security
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
// --pallet=security
// --extrinsic=*
// --steps=100
// --repeat=10
// --wasm-execution=compiled
// --output=pallets/security/src/default_weights.rs
// --template=.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for security.
pub trait WeightInfo {
	fn on_initialize() -> Weight;
	fn set_parachain_status() -> Weight;
	fn insert_parachain_error() -> Weight;
	fn remove_parachain_error() -> Weight;
	
}

/// Weights for security using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn on_initialize() -> Weight {
		// Minimum execution time: 1_000 nanoseconds.
		Weight::from_ref_time(1_000_000_u64)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn set_parachain_status() -> Weight {
		Weight::from_ref_time(0_u64)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn insert_parachain_error() -> Weight {
		Weight::from_ref_time(0_u64)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn remove_parachain_error() -> Weight {
		Weight::from_ref_time(0_u64)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn on_initialize() -> Weight {
		// Minimum execution time: 1_000 nanoseconds.
		Weight::from_ref_time(1_000_000_u64)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn set_parachain_status() -> Weight {
		Weight::from_ref_time(0_u64)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn insert_parachain_error() -> Weight {
		Weight::from_ref_time(0_u64)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn remove_parachain_error() -> Weight {
		Weight::from_ref_time(0_u64)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	
}