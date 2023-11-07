
//! Autogenerated weights for reward_distribution
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-10-17, STEPS: `100`, REPEAT: `10`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `pop-os`, CPU: `Intel(R) Core(TM) i7-7700K CPU @ 4.20GHz`
//! EXECUTION: None, WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/spacewalk-standalone
// benchmark
// pallet
// --chain=dev
// --pallet=reward-distribution
// --extrinsic=*
// --steps=100
// --repeat=10
// --wasm-execution=compiled
// --output=pallets/reward-distribution/src/default_weights.rs
// --template=./.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for reward_distribution.
pub trait WeightInfo {
	fn set_reward_per_block() -> Weight;
	fn collect_reward() -> Weight;
	fn on_initialize() -> Weight;
}

/// Weights for reward_distribution using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: RewardDistribution RewardsAdaptedAt (r:0 w:1)
	/// Proof: RewardDistribution RewardsAdaptedAt (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: RewardDistribution RewardPerBlock (r:0 w:1)
	/// Proof: RewardDistribution RewardPerBlock (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	fn set_reward_per_block() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 11_898_000 picoseconds.
		Weight::from_parts(12_110_000, 0)
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: VaultRewards Stake (r:1 w:0)
	/// Proof: VaultRewards Stake (max_values: None, max_size: Some(202), added: 2677, mode: MaxEncodedLen)
	/// Storage: VaultRewards RewardPerToken (r:1 w:0)
	/// Proof: VaultRewards RewardPerToken (max_values: None, max_size: Some(140), added: 2615, mode: MaxEncodedLen)
	/// Storage: VaultRewards RewardTally (r:1 w:1)
	/// Proof: VaultRewards RewardTally (max_values: None, max_size: Some(264), added: 2739, mode: MaxEncodedLen)
	/// Storage: VaultRewards TotalRewards (r:1 w:1)
	/// Proof: VaultRewards TotalRewards (max_values: None, max_size: Some(78), added: 2553, mode: MaxEncodedLen)
	/// Storage: VaultStaking Nonce (r:1 w:0)
	/// Proof Skipped: VaultStaking Nonce (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	/// Proof Skipped: VaultStaking TotalCurrentStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardPerToken (r:1 w:1)
	/// Proof Skipped: VaultStaking RewardPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalRewards (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalRewards (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking Stake (r:1 w:1)
	/// Proof Skipped: VaultStaking Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashPerToken (r:1 w:0)
	/// Proof Skipped: VaultStaking SlashPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashTally (r:1 w:1)
	/// Proof Skipped: VaultStaking SlashTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalStake (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardTally (r:1 w:1)
	/// Proof Skipped: VaultStaking RewardTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn collect_reward() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1118`
		//  Estimated: `59384`
		// Minimum execution time: 63_270_000 picoseconds.
		Weight::from_parts(64_571_000, 59384)
			.saturating_add(T::DbWeight::get().reads(14_u64))
			.saturating_add(T::DbWeight::get().writes(8_u64))
	}
	/// Storage: RewardDistribution RewardPerBlock (r:1 w:0)
	/// Proof: RewardDistribution RewardPerBlock (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	fn on_initialize() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `232`
		//  Estimated: `3218`
		// Minimum execution time: 16_761_000 picoseconds.
		Weight::from_parts(16_934_000, 3218)
			.saturating_add(T::DbWeight::get().reads(2_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: RewardDistribution RewardsAdaptedAt (r:0 w:1)
	/// Proof: RewardDistribution RewardsAdaptedAt (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: RewardDistribution RewardPerBlock (r:0 w:1)
	/// Proof: RewardDistribution RewardPerBlock (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	fn set_reward_per_block() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 11_898_000 picoseconds.
		Weight::from_parts(12_110_000, 0)
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: VaultRewards Stake (r:1 w:0)
	/// Proof: VaultRewards Stake (max_values: None, max_size: Some(202), added: 2677, mode: MaxEncodedLen)
	/// Storage: VaultRewards RewardPerToken (r:1 w:0)
	/// Proof: VaultRewards RewardPerToken (max_values: None, max_size: Some(140), added: 2615, mode: MaxEncodedLen)
	/// Storage: VaultRewards RewardTally (r:1 w:1)
	/// Proof: VaultRewards RewardTally (max_values: None, max_size: Some(264), added: 2739, mode: MaxEncodedLen)
	/// Storage: VaultRewards TotalRewards (r:1 w:1)
	/// Proof: VaultRewards TotalRewards (max_values: None, max_size: Some(78), added: 2553, mode: MaxEncodedLen)
	/// Storage: VaultStaking Nonce (r:1 w:0)
	/// Proof Skipped: VaultStaking Nonce (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	/// Proof Skipped: VaultStaking TotalCurrentStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardPerToken (r:1 w:1)
	/// Proof Skipped: VaultStaking RewardPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalRewards (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalRewards (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking Stake (r:1 w:1)
	/// Proof Skipped: VaultStaking Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashPerToken (r:1 w:0)
	/// Proof Skipped: VaultStaking SlashPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashTally (r:1 w:1)
	/// Proof Skipped: VaultStaking SlashTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalStake (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardTally (r:1 w:1)
	/// Proof Skipped: VaultStaking RewardTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn collect_reward() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1118`
		//  Estimated: `59384`
		// Minimum execution time: 63_270_000 picoseconds.
		Weight::from_parts(64_571_000, 59384)
			.saturating_add(RocksDbWeight::get().reads(14_u64))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
	}
	/// Storage: RewardDistribution RewardPerBlock (r:1 w:0)
	/// Proof: RewardDistribution RewardPerBlock (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	fn on_initialize() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `232`
		//  Estimated: `3218`
		// Minimum execution time: 16_761_000 picoseconds.
		Weight::from_parts(16_934_000, 3218)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
	}
}