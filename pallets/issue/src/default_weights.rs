
//! Autogenerated weights for issue
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2024-05-23, STEPS: `100`, REPEAT: `10`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `Bogdans-M2-MacBook-Pro.local`, CPU: `<UNKNOWN>`
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
// --template=./.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

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
	/// Storage: Issue LimitVolumeAmount (r:1 w:0)
	/// Proof: Issue LimitVolumeAmount (max_values: Some(1), max_size: Some(17), added: 512, mode: MaxEncodedLen)
	/// Storage: VaultRegistry Vaults (r:1 w:1)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Fee IssueGriefingCollateral (r:1 w:0)
	/// Proof Skipped: Fee IssueGriefingCollateral (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:1 w:1)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(150), added: 2625, mode: MaxEncodedLen)
	/// Storage: Issue IssueMinimumTransferAmount (r:1 w:0)
	/// Proof: Issue IssueMinimumTransferAmount (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	/// Proof Skipped: VaultRegistry SecureCollateralThreshold (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking Nonce (r:1 w:0)
	/// Proof Skipped: VaultStaking Nonce (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	/// Proof Skipped: VaultStaking TotalCurrentStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: Fee IssueFee (r:1 w:0)
	/// Proof Skipped: Fee IssueFee (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Security Nonce (r:1 w:1)
	/// Proof Skipped: Security Nonce (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: System ParentHash (r:1 w:0)
	/// Proof: System ParentHash (max_values: Some(1), max_size: Some(32), added: 527, mode: MaxEncodedLen)
	/// Storage: VaultRegistry VaultStellarPublicKey (r:1 w:0)
	/// Proof Skipped: VaultRegistry VaultStellarPublicKey (max_values: None, max_size: None, mode: Measured)
	/// Storage: Security ActiveBlockCount (r:1 w:0)
	/// Proof Skipped: Security ActiveBlockCount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Issue IssuePeriod (r:1 w:0)
	/// Proof: Issue IssuePeriod (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: Issue IssueRequests (r:0 w:1)
	/// Proof: Issue IssueRequests (max_values: None, max_size: Some(339), added: 2814, mode: MaxEncodedLen)
	fn request_issue() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2192`
		//  Estimated: `5657`
		// Minimum execution time: 56_000_000 picoseconds.
		Weight::from_parts(57_000_000, 5657)
			.saturating_add(T::DbWeight::get().reads(15_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	/// Storage: Issue IssueRequests (r:1 w:1)
	/// Proof: Issue IssueRequests (max_values: None, max_size: Some(339), added: 2814, mode: MaxEncodedLen)
	/// Storage: StellarRelay NewValidatorsEnactmentBlockHeight (r:1 w:0)
	/// Proof: StellarRelay NewValidatorsEnactmentBlockHeight (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: StellarRelay Validators (r:1 w:0)
	/// Proof: StellarRelay Validators (max_values: Some(1), max_size: Some(70382), added: 70877, mode: MaxEncodedLen)
	/// Storage: StellarRelay Organizations (r:1 w:0)
	/// Proof: StellarRelay Organizations (max_values: Some(1), max_size: Some(37232), added: 37727, mode: MaxEncodedLen)
	/// Storage: VaultRegistry Vaults (r:1 w:1)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: Fee IssueFee (r:1 w:0)
	/// Proof Skipped: Fee IssueFee (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Issue LimitVolumeAmount (r:1 w:0)
	/// Proof: Issue LimitVolumeAmount (max_values: Some(1), max_size: Some(17), added: 512, mode: MaxEncodedLen)
	/// Storage: VaultRewards TotalStake (r:2 w:0)
	/// Proof: VaultRewards TotalStake (max_values: None, max_size: Some(78), added: 2553, mode: MaxEncodedLen)
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	fn execute_issue() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2190`
		//  Estimated: `71867`
		// Minimum execution time: 3_998_000_000 picoseconds.
		Weight::from_parts(4_166_000_000, 71867)
			.saturating_add(T::DbWeight::get().reads(10_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: Issue IssueRequests (r:1 w:1)
	/// Proof: Issue IssueRequests (max_values: None, max_size: Some(339), added: 2814, mode: MaxEncodedLen)
	/// Storage: Issue IssuePeriod (r:1 w:0)
	/// Proof: Issue IssuePeriod (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: Security ActiveBlockCount (r:1 w:0)
	/// Proof Skipped: Security ActiveBlockCount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: VaultRegistry Vaults (r:1 w:1)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	fn cancel_issue() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1199`
		//  Estimated: `4664`
		// Minimum execution time: 25_000_000 picoseconds.
		Weight::from_parts(27_000_000, 4664)
			.saturating_add(T::DbWeight::get().reads(4_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: Issue IssuePeriod (r:0 w:1)
	/// Proof: Issue IssuePeriod (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	fn set_issue_period() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_000_000 picoseconds.
		Weight::from_parts(4_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: Issue LimitVolumeAmount (r:0 w:1)
	/// Proof: Issue LimitVolumeAmount (max_values: Some(1), max_size: Some(17), added: 512, mode: MaxEncodedLen)
	/// Storage: Issue LimitVolumeCurrencyId (r:0 w:1)
	/// Proof: Issue LimitVolumeCurrencyId (max_values: Some(1), max_size: Some(46), added: 541, mode: MaxEncodedLen)
	/// Storage: Issue IntervalLength (r:0 w:1)
	/// Proof: Issue IntervalLength (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	fn rate_limit_update() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 5_000_000 picoseconds.
		Weight::from_parts(5_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	/// Storage: Issue IssueMinimumTransferAmount (r:0 w:1)
	/// Proof: Issue IssueMinimumTransferAmount (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	fn minimum_transfer_amount_update() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_000_000 picoseconds.
		Weight::from_parts(5_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: Issue LimitVolumeAmount (r:1 w:0)
	/// Proof: Issue LimitVolumeAmount (max_values: Some(1), max_size: Some(17), added: 512, mode: MaxEncodedLen)
	/// Storage: VaultRegistry Vaults (r:1 w:1)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Fee IssueGriefingCollateral (r:1 w:0)
	/// Proof Skipped: Fee IssueGriefingCollateral (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:1 w:1)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(150), added: 2625, mode: MaxEncodedLen)
	/// Storage: Issue IssueMinimumTransferAmount (r:1 w:0)
	/// Proof: Issue IssueMinimumTransferAmount (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	/// Proof Skipped: VaultRegistry SecureCollateralThreshold (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking Nonce (r:1 w:0)
	/// Proof Skipped: VaultStaking Nonce (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	/// Proof Skipped: VaultStaking TotalCurrentStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: Fee IssueFee (r:1 w:0)
	/// Proof Skipped: Fee IssueFee (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Security Nonce (r:1 w:1)
	/// Proof Skipped: Security Nonce (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: System ParentHash (r:1 w:0)
	/// Proof: System ParentHash (max_values: Some(1), max_size: Some(32), added: 527, mode: MaxEncodedLen)
	/// Storage: VaultRegistry VaultStellarPublicKey (r:1 w:0)
	/// Proof Skipped: VaultRegistry VaultStellarPublicKey (max_values: None, max_size: None, mode: Measured)
	/// Storage: Security ActiveBlockCount (r:1 w:0)
	/// Proof Skipped: Security ActiveBlockCount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Issue IssuePeriod (r:1 w:0)
	/// Proof: Issue IssuePeriod (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: Issue IssueRequests (r:0 w:1)
	/// Proof: Issue IssueRequests (max_values: None, max_size: Some(339), added: 2814, mode: MaxEncodedLen)
	fn request_issue() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2192`
		//  Estimated: `5657`
		// Minimum execution time: 56_000_000 picoseconds.
		Weight::from_parts(57_000_000, 5657)
			.saturating_add(RocksDbWeight::get().reads(15_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: Issue IssueRequests (r:1 w:1)
	/// Proof: Issue IssueRequests (max_values: None, max_size: Some(339), added: 2814, mode: MaxEncodedLen)
	/// Storage: StellarRelay NewValidatorsEnactmentBlockHeight (r:1 w:0)
	/// Proof: StellarRelay NewValidatorsEnactmentBlockHeight (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: StellarRelay Validators (r:1 w:0)
	/// Proof: StellarRelay Validators (max_values: Some(1), max_size: Some(70382), added: 70877, mode: MaxEncodedLen)
	/// Storage: StellarRelay Organizations (r:1 w:0)
	/// Proof: StellarRelay Organizations (max_values: Some(1), max_size: Some(37232), added: 37727, mode: MaxEncodedLen)
	/// Storage: VaultRegistry Vaults (r:1 w:1)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: Fee IssueFee (r:1 w:0)
	/// Proof Skipped: Fee IssueFee (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Issue LimitVolumeAmount (r:1 w:0)
	/// Proof: Issue LimitVolumeAmount (max_values: Some(1), max_size: Some(17), added: 512, mode: MaxEncodedLen)
	/// Storage: VaultRewards TotalStake (r:2 w:0)
	/// Proof: VaultRewards TotalStake (max_values: None, max_size: Some(78), added: 2553, mode: MaxEncodedLen)
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	fn execute_issue() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2190`
		//  Estimated: `71867`
		// Minimum execution time: 3_998_000_000 picoseconds.
		Weight::from_parts(4_166_000_000, 71867)
			.saturating_add(RocksDbWeight::get().reads(10_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: Issue IssueRequests (r:1 w:1)
	/// Proof: Issue IssueRequests (max_values: None, max_size: Some(339), added: 2814, mode: MaxEncodedLen)
	/// Storage: Issue IssuePeriod (r:1 w:0)
	/// Proof: Issue IssuePeriod (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: Security ActiveBlockCount (r:1 w:0)
	/// Proof Skipped: Security ActiveBlockCount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: VaultRegistry Vaults (r:1 w:1)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	fn cancel_issue() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1199`
		//  Estimated: `4664`
		// Minimum execution time: 25_000_000 picoseconds.
		Weight::from_parts(27_000_000, 4664)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: Issue IssuePeriod (r:0 w:1)
	/// Proof: Issue IssuePeriod (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	fn set_issue_period() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_000_000 picoseconds.
		Weight::from_parts(4_000_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: Issue LimitVolumeAmount (r:0 w:1)
	/// Proof: Issue LimitVolumeAmount (max_values: Some(1), max_size: Some(17), added: 512, mode: MaxEncodedLen)
	/// Storage: Issue LimitVolumeCurrencyId (r:0 w:1)
	/// Proof: Issue LimitVolumeCurrencyId (max_values: Some(1), max_size: Some(46), added: 541, mode: MaxEncodedLen)
	/// Storage: Issue IntervalLength (r:0 w:1)
	/// Proof: Issue IntervalLength (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	fn rate_limit_update() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 5_000_000 picoseconds.
		Weight::from_parts(5_000_000, 0)
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	/// Storage: Issue IssueMinimumTransferAmount (r:0 w:1)
	/// Proof: Issue IssueMinimumTransferAmount (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	fn minimum_transfer_amount_update() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_000_000 picoseconds.
		Weight::from_parts(5_000_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}