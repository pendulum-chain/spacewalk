
//! Autogenerated weights for `clients_info`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-10-13, STEPS: `20`, REPEAT: `10`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `Bs-MacBook-Pro.local`, CPU: `<UNKNOWN>`
//! EXECUTION: None, WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/spacewalk-standalone
// benchmark
// pallet
// --chain
// dev
// --pallet=clients-info
// --extrinsic
// *
// --steps
// 20
// --repeat
// 10
// --output
// default_weights.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions needed for clients_info.
pub trait WeightInfo {
	fn set_current_client_release(n: u32, u: u32, ) -> Weight;
	fn set_pending_client_release(n: u32, u: u32, ) -> Weight;
	fn authorize_account() -> Weight;
	fn deauthorize_account() -> Weight;
}

/// Weight functions for `clients_info`.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: ClientsInfo AuthorizedAccounts (r:1 w:0)
	/// Proof: ClientsInfo AuthorizedAccounts (max_values: None, max_size: Some(48), added: 2523, mode: MaxEncodedLen)
	/// Storage: ClientsInfo CurrentClientReleases (r:0 w:1)
	/// Proof: ClientsInfo CurrentClientReleases (max_values: None, max_size: Some(562), added: 3037, mode: MaxEncodedLen)
	/// The range of component `n` is `[0, 255]`.
	/// The range of component `u` is `[0, 255]`.
	fn set_current_client_release(n: u32, _u: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `77`
		//  Estimated: `3513`
		// Minimum execution time: 6_000_000 picoseconds.
		Weight::from_parts(6_960_200, 0)
			.saturating_add(Weight::from_parts(0, 3513))
			// Standard Error: 394
			.saturating_add(Weight::from_parts(3_078, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: ClientsInfo AuthorizedAccounts (r:1 w:0)
	/// Proof: ClientsInfo AuthorizedAccounts (max_values: None, max_size: Some(48), added: 2523, mode: MaxEncodedLen)
	/// Storage: ClientsInfo PendingClientReleases (r:0 w:1)
	/// Proof: ClientsInfo PendingClientReleases (max_values: None, max_size: Some(562), added: 3037, mode: MaxEncodedLen)
	/// The range of component `n` is `[0, 255]`.
	/// The range of component `u` is `[0, 255]`.
	fn set_pending_client_release(n: u32, u: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `77`
		//  Estimated: `3513`
		// Minimum execution time: 6_000_000 picoseconds.
		Weight::from_parts(6_567_981, 0)
			.saturating_add(Weight::from_parts(0, 3513))
			// Standard Error: 405
			.saturating_add(Weight::from_parts(3_456, 0).saturating_mul(n.into()))
			// Standard Error: 405
			.saturating_add(Weight::from_parts(1_017, 0).saturating_mul(u.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: ClientsInfo AuthorizedAccounts (r:2 w:1)
	/// Proof: ClientsInfo AuthorizedAccounts (max_values: None, max_size: Some(48), added: 2523, mode: MaxEncodedLen)
	fn authorize_account() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `77`
		//  Estimated: `6036`
		// Minimum execution time: 6_000_000 picoseconds.
		Weight::from_parts(7_000_000, 0)
			.saturating_add(Weight::from_parts(0, 6036))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: ClientsInfo AuthorizedAccounts (r:2 w:1)
	/// Proof: ClientsInfo AuthorizedAccounts (max_values: None, max_size: Some(48), added: 2523, mode: MaxEncodedLen)
	fn deauthorize_account() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `134`
		//  Estimated: `6036`
		// Minimum execution time: 8_000_000 picoseconds.
		Weight::from_parts(9_000_000, 0)
			.saturating_add(Weight::from_parts(0, 6036))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
