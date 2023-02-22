
//! Autogenerated weights for fee
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
// --pallet=fee
// --extrinsic=*
// --steps=100
// --repeat=10
// --wasm-execution=compiled
// --output=pallets/fee/src/default_weights.rs
// --template=.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for fee.
pub trait WeightInfo {
	fn withdraw_rewards() -> Weight;
	fn set_issue_fee() -> Weight;
	fn set_issue_griefing_collateral() -> Weight;
	fn set_redeem_fee() -> Weight;
	fn set_premium_redeem_fee() -> Weight;
	fn set_punishment_fee() -> Weight;
	fn set_replace_griefing_collateral() -> Weight;
}

/// Weights for fee using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: VaultRewards Stake (r:1 w:0)
	// Storage: VaultRewards RewardPerToken (r:1 w:0)
	// Storage: VaultRewards RewardTally (r:1 w:1)
	// Storage: VaultRewards TotalRewards (r:1 w:1)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	// Storage: VaultStaking RewardPerToken (r:1 w:0)
	// Storage: VaultStaking RewardTally (r:1 w:1)
	// Storage: VaultStaking TotalRewards (r:1 w:1)
	fn withdraw_rewards() -> Weight {
		// Minimum execution time: 49_000 nanoseconds.
		Weight::from_ref_time(49_000_000u64)
			.saturating_add(T::DbWeight::get().reads(13u64))
			.saturating_add(T::DbWeight::get().writes(7u64))
	}
	// Storage: Fee IssueFee (r:0 w:1)
	fn set_issue_fee() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(3_000_000u64)
			.saturating_add(T::DbWeight::get().writes(1u64))
	}
	// Storage: Fee IssueGriefingCollateral (r:0 w:1)
	fn set_issue_griefing_collateral() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(4_000_000u64)
			.saturating_add(T::DbWeight::get().writes(1u64))
	}
	// Storage: Fee RedeemFee (r:0 w:1)
	fn set_redeem_fee() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(3_000_000u64)
			.saturating_add(T::DbWeight::get().writes(1u64))
	}
	// Storage: Fee PremiumRedeemFee (r:0 w:1)
	fn set_premium_redeem_fee() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(3_000_000u64)
			.saturating_add(T::DbWeight::get().writes(1u64))
	}
	// Storage: Fee PunishmentFee (r:0 w:1)
	fn set_punishment_fee() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(3_000_000u64)
			.saturating_add(T::DbWeight::get().writes(1u64))
	}
	// Storage: Fee ReplaceGriefingCollateral (r:0 w:1)
	fn set_replace_griefing_collateral() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(3_000_000u64)
			.saturating_add(T::DbWeight::get().writes(1u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: VaultRewards Stake (r:1 w:0)
	// Storage: VaultRewards RewardPerToken (r:1 w:0)
	// Storage: VaultRewards RewardTally (r:1 w:1)
	// Storage: VaultRewards TotalRewards (r:1 w:1)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	// Storage: VaultStaking RewardPerToken (r:1 w:0)
	// Storage: VaultStaking RewardTally (r:1 w:1)
	// Storage: VaultStaking TotalRewards (r:1 w:1)
	fn withdraw_rewards() -> Weight {
		// Minimum execution time: 49_000 nanoseconds.
		Weight::from_ref_time(49_000_000u64)
			.saturating_add(RocksDbWeight::get().reads(13u64))
			.saturating_add(RocksDbWeight::get().writes(7u64))
	}
	// Storage: Fee IssueFee (r:0 w:1)
	fn set_issue_fee() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(3_000_000u64)
			.saturating_add(RocksDbWeight::get().writes(1u64))
	}
	// Storage: Fee IssueGriefingCollateral (r:0 w:1)
	fn set_issue_griefing_collateral() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(4_000_000u64)
			.saturating_add(RocksDbWeight::get().writes(1u64))
	}
	// Storage: Fee RedeemFee (r:0 w:1)
	fn set_redeem_fee() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(3_000_000u64)
			.saturating_add(RocksDbWeight::get().writes(1u64))
	}
	// Storage: Fee PremiumRedeemFee (r:0 w:1)
	fn set_premium_redeem_fee() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(3_000_000u64)
			.saturating_add(RocksDbWeight::get().writes(1u64))
	}
	// Storage: Fee PunishmentFee (r:0 w:1)
	fn set_punishment_fee() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(3_000_000u64)
			.saturating_add(RocksDbWeight::get().writes(1u64))
	}
	// Storage: Fee ReplaceGriefingCollateral (r:0 w:1)
	fn set_replace_griefing_collateral() -> Weight {
		// Minimum execution time: 3_000 nanoseconds.
		Weight::from_ref_time(3_000_000u64)
			.saturating_add(RocksDbWeight::get().writes(1u64))
	}
}