#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use frame_support::{traits::Get, weights::{constants::RocksDbWeight, Weight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for security.
pub trait WeightInfo {
	fn on_initialize() -> Weight;
	fn set_parachain_status() -> Weight;
	fn insert_parachain_error() -> Weight;
	fn remove_parachain_error() -> Weight;
	
}

/// Weights for security using the Substrate node and recommended hardware.
/// The respective weights were picked manually based on the functions each extrinsic uses.
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