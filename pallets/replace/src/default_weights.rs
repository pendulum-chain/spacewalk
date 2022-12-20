
//! Autogenerated weights for replace
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
// --pallet=replace
// --extrinsic=*
// --steps=100
// --repeat=10
// --wasm-execution=compiled
// --output=pallets/replace/src/default_weights.rs
// --template=.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for replace.
pub trait WeightInfo {
	fn request_replace() -> Weight;
	fn withdraw_replace() -> Weight;
	fn accept_replace() -> Weight;
	fn execute_replace() -> Weight;
	fn cancel_replace() -> Weight;
	fn set_replace_period() -> Weight;
}

/// Weights for replace using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Nomination Vaults (r:1 w:0)
	// Storage: Replace ReplaceBtcDustValue (r:1 w:0)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: Fee ReplaceGriefingCollateral (r:1 w:0)
	// Storage: Tokens Accounts (r:1 w:1)
	fn request_replace() -> Weight {
		// Minimum execution time: 39_000 nanoseconds.
		Weight::from_ref_time(39_000_000_u64)
			.saturating_add(T::DbWeight::get().reads(7_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn withdraw_replace() -> Weight {
		// Minimum execution time: 17_000 nanoseconds.
		Weight::from_ref_time(18_000_000_u64)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	// Storage: VaultRegistry Vaults (r:2 w:2)
	// Storage: Replace ReplaceBtcDustValue (r:1 w:0)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: VaultRegistry SystemCollateralCeiling (r:1 w:0)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	// Storage: VaultStaking RewardTally (r:2 w:2)
	// Storage: VaultStaking RewardPerToken (r:2 w:0)
	// Storage: VaultRewards Stake (r:1 w:1)
	// Storage: VaultRewards TotalStake (r:1 w:1)
	// Storage: VaultRewards RewardTally (r:2 w:2)
	// Storage: VaultRewards RewardPerToken (r:2 w:0)
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: Security Nonce (r:1 w:1)
	// Storage: System ParentHash (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: Replace ReplacePeriod (r:1 w:0)
	// Storage: Replace ReplaceRequests (r:0 w:1)
	fn accept_replace() -> Weight {
		// Minimum execution time: 108_000 nanoseconds.
		Weight::from_ref_time(110_000_000_u64)
			.saturating_add(T::DbWeight::get().reads(29_u64))
			.saturating_add(T::DbWeight::get().writes(16_u64))
	}
	// Storage: Replace ReplaceRequests (r:1 w:1)
	// Storage: StellarRelay IsPublicNetwork (r:1 w:0)
	// Storage: StellarRelay Validators (r:1 w:0)
	// Storage: StellarRelay Organizations (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:2 w:2)
	// Storage: VaultRewards Stake (r:1 w:0)
	fn execute_replace() -> Weight {
		// Minimum execution time: 4_076_000 nanoseconds.
		Weight::from_ref_time(4_104_000_000_u64)
			.saturating_add(T::DbWeight::get().reads(7_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	// Storage: Replace ReplaceRequests (r:1 w:1)
	// Storage: Replace ReplacePeriod (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:2 w:2)
	// Storage: VaultRewards Stake (r:1 w:1)
	// Storage: VaultRewards TotalStake (r:1 w:1)
	// Storage: VaultRewards RewardTally (r:2 w:2)
	// Storage: VaultRewards RewardPerToken (r:2 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	fn cancel_replace() -> Weight {
		// Minimum execution time: 73_000 nanoseconds.
		Weight::from_ref_time(74_000_000_u64)
			.saturating_add(T::DbWeight::get().reads(21_u64))
			.saturating_add(T::DbWeight::get().writes(11_u64))
	}
	// Storage: Replace ReplacePeriod (r:0 w:1)
	fn set_replace_period() -> Weight {
		// Minimum execution time: 7_000 nanoseconds.
		Weight::from_ref_time(8_000_000_u64)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Nomination Vaults (r:1 w:0)
	// Storage: Replace ReplaceBtcDustValue (r:1 w:0)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: Fee ReplaceGriefingCollateral (r:1 w:0)
	// Storage: Tokens Accounts (r:1 w:1)
	fn request_replace() -> Weight {
		// Minimum execution time: 39_000 nanoseconds.
		Weight::from_ref_time(39_000_000_u64)
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn withdraw_replace() -> Weight {
		// Minimum execution time: 17_000 nanoseconds.
		Weight::from_ref_time(18_000_000_u64)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	// Storage: VaultRegistry Vaults (r:2 w:2)
	// Storage: Replace ReplaceBtcDustValue (r:1 w:0)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: VaultRegistry SystemCollateralCeiling (r:1 w:0)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	// Storage: VaultStaking RewardTally (r:2 w:2)
	// Storage: VaultStaking RewardPerToken (r:2 w:0)
	// Storage: VaultRewards Stake (r:1 w:1)
	// Storage: VaultRewards TotalStake (r:1 w:1)
	// Storage: VaultRewards RewardTally (r:2 w:2)
	// Storage: VaultRewards RewardPerToken (r:2 w:0)
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: Security Nonce (r:1 w:1)
	// Storage: System ParentHash (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: Replace ReplacePeriod (r:1 w:0)
	// Storage: Replace ReplaceRequests (r:0 w:1)
	fn accept_replace() -> Weight {
		// Minimum execution time: 108_000 nanoseconds.
		Weight::from_ref_time(110_000_000_u64)
			.saturating_add(RocksDbWeight::get().reads(29_u64))
			.saturating_add(RocksDbWeight::get().writes(16_u64))
	}
	// Storage: Replace ReplaceRequests (r:1 w:1)
	// Storage: StellarRelay IsPublicNetwork (r:1 w:0)
	// Storage: StellarRelay Validators (r:1 w:0)
	// Storage: StellarRelay Organizations (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:2 w:2)
	// Storage: VaultRewards Stake (r:1 w:0)
	fn execute_replace() -> Weight {
		// Minimum execution time: 4_076_000 nanoseconds.
		Weight::from_ref_time(4_104_000_000_u64)
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	// Storage: Replace ReplaceRequests (r:1 w:1)
	// Storage: Replace ReplacePeriod (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:2 w:2)
	// Storage: VaultRewards Stake (r:1 w:1)
	// Storage: VaultRewards TotalStake (r:1 w:1)
	// Storage: VaultRewards RewardTally (r:2 w:2)
	// Storage: VaultRewards RewardPerToken (r:2 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	fn cancel_replace() -> Weight {
		// Minimum execution time: 73_000 nanoseconds.
		Weight::from_ref_time(74_000_000_u64)
			.saturating_add(RocksDbWeight::get().reads(21_u64))
			.saturating_add(RocksDbWeight::get().writes(11_u64))
	}
	// Storage: Replace ReplacePeriod (r:0 w:1)
	fn set_replace_period() -> Weight {
		// Minimum execution time: 7_000 nanoseconds.
		Weight::from_ref_time(8_000_000_u64)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}