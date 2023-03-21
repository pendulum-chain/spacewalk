
//! Autogenerated weights for issue
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-03-09, STEPS: `100`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `MacBook-Pro`, CPU: `<UNKNOWN>`
//! EXECUTION: None, WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/spacewalk-standalone
// benchmark
// pallet
// --chain=dev
// --pallet=issue
// --extrinsic=*
// --steps=100
// --repeat=10
// --wasm-execution=compiled
// --output=pallets/issue/src/default_weights.rs
// --template=.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for issue.
pub trait WeightInfo {
	fn request_issue() -> Weight;
	fn execute_issue() -> Weight;
	fn cancel_issue() -> Weight;
	fn set_issue_period() -> Weight;
	fn rate_limit_update() -> Weight;
	fn minimum_transfer_amount_update() -> Weight;
}

/// Weights for issue using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: Issue LimitVolumeAmount (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Fee IssueGriefingCollateral (r:1 w:0)
	// Storage: Issue IssueMinimumTransferAmount (r:1 w:0)
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: Fee IssueFee (r:1 w:0)
	// Storage: Security Nonce (r:1 w:1)
	// Storage: System ParentHash (r:1 w:0)
	// Storage: VaultRegistry VaultStellarPublicKey (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: Issue IssuePeriod (r:1 w:0)
	// Storage: Issue IssueRequests (r:0 w:1)
	fn request_issue() -> Weight {
		// Minimum execution time: 62_000 nanoseconds.
		Weight::from_ref_time(63_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(14 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Issue IssueRequests (r:1 w:1)
	// Storage: StellarRelay IsPublicNetwork (r:1 w:0)
	// Storage: StellarRelay NewValidatorsEnactmentBlockHeight (r:1 w:0)
	// Storage: StellarRelay Validators (r:1 w:0)
	// Storage: StellarRelay Organizations (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Fee IssueFee (r:1 w:0)
	// Storage: VaultRewards Stake (r:1 w:0)
	// Storage: Issue LimitVolumeAmount (r:1 w:0)
	// Storage: VaultRewards TotalStake (r:1 w:0)
	fn execute_issue() -> Weight {
		// Minimum execution time: 4_254_000 nanoseconds.
		Weight::from_ref_time(4_384_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(10 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Issue IssueRequests (r:1 w:1)
	// Storage: Issue IssuePeriod (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn cancel_issue() -> Weight {
		// Minimum execution time: 34_000 nanoseconds.
		Weight::from_ref_time(36_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Issue IssuePeriod (r:0 w:1)
	fn set_issue_period() -> Weight {
		// Minimum execution time: 11_000 nanoseconds.
		Weight::from_ref_time(12_000_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Issue LimitVolumeAmount (r:0 w:1)
	// Storage: Issue LimitVolumeCurrencyId (r:0 w:1)
	// Storage: Issue IntervalLength (r:0 w:1)
	fn rate_limit_update() -> Weight {
		// Minimum execution time: 13_000 nanoseconds.
		Weight::from_ref_time(13_000_000 as u64)
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Issue IssueMinimumTransferAmount (r:0 w:1)
	fn minimum_transfer_amount_update() -> Weight {
		// Minimum execution time: 11_000 nanoseconds.
		Weight::from_ref_time(14_000_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: Issue LimitVolumeAmount (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Fee IssueGriefingCollateral (r:1 w:0)
	// Storage: Issue IssueMinimumTransferAmount (r:1 w:0)
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: Fee IssueFee (r:1 w:0)
	// Storage: Security Nonce (r:1 w:1)
	// Storage: System ParentHash (r:1 w:0)
	// Storage: VaultRegistry VaultStellarPublicKey (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: Issue IssuePeriod (r:1 w:0)
	// Storage: Issue IssueRequests (r:0 w:1)
	fn request_issue() -> Weight {
		// Minimum execution time: 62_000 nanoseconds.
		Weight::from_ref_time(63_000_000 as u64)
			.saturating_add(RocksDbWeight::get().reads(14 as u64))
			.saturating_add(RocksDbWeight::get().writes(3 as u64))
	}
	// Storage: Issue IssueRequests (r:1 w:1)
	// Storage: StellarRelay IsPublicNetwork (r:1 w:0)
	// Storage: StellarRelay NewValidatorsEnactmentBlockHeight (r:1 w:0)
	// Storage: StellarRelay Validators (r:1 w:0)
	// Storage: StellarRelay Organizations (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Fee IssueFee (r:1 w:0)
	// Storage: VaultRewards Stake (r:1 w:0)
	// Storage: Issue LimitVolumeAmount (r:1 w:0)
	// Storage: VaultRewards TotalStake (r:1 w:0)
	fn execute_issue() -> Weight {
		// Minimum execution time: 4_254_000 nanoseconds.
		Weight::from_ref_time(4_384_000_000 as u64)
			.saturating_add(RocksDbWeight::get().reads(10 as u64))
			.saturating_add(RocksDbWeight::get().writes(2 as u64))
	}
	// Storage: Issue IssueRequests (r:1 w:1)
	// Storage: Issue IssuePeriod (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn cancel_issue() -> Weight {
		// Minimum execution time: 34_000 nanoseconds.
		Weight::from_ref_time(36_000_000 as u64)
			.saturating_add(RocksDbWeight::get().reads(4 as u64))
			.saturating_add(RocksDbWeight::get().writes(2 as u64))
	}
	// Storage: Issue IssuePeriod (r:0 w:1)
	fn set_issue_period() -> Weight {
		// Minimum execution time: 11_000 nanoseconds.
		Weight::from_ref_time(12_000_000 as u64)
			.saturating_add(RocksDbWeight::get().writes(1 as u64))
	}
	// Storage: Issue LimitVolumeAmount (r:0 w:1)
	// Storage: Issue LimitVolumeCurrencyId (r:0 w:1)
	// Storage: Issue IntervalLength (r:0 w:1)
	fn rate_limit_update() -> Weight {
		// Minimum execution time: 13_000 nanoseconds.
		Weight::from_ref_time(13_000_000 as u64)
			.saturating_add(RocksDbWeight::get().writes(3 as u64))
	}
	// Storage: Issue IssueMinimumTransferAmount (r:0 w:1)
	fn minimum_transfer_amount_update() -> Weight {
		// Minimum execution time: 11_000 nanoseconds.
		Weight::from_ref_time(14_000_000 as u64)
			.saturating_add(RocksDbWeight::get().writes(1 as u64))
	}
}