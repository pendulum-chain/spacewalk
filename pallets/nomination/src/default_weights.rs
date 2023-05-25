
//! Autogenerated weights for `nomination`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-05-24, STEPS: `100`, REPEAT: `10`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `Bs-MacBook-Pro.local`, CPU: `<UNKNOWN>`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/spacewalk-standalone
// benchmark
// pallet
// --chain
// dev
// --execution=wasm
// --wasm-execution=compiled
// --pallet
// *
// --extrinsic
// *
// --steps=100
// --repeat=10
// --output
// pallets

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `nomination`.
pub trait WeightInfo {
	fn set_nomination_enabled() -> Weight;
	fn opt_in_to_nomination() -> Weight;
	fn opt_out_of_nomination() -> Weight;
	fn deposit_collateral() -> Weight;
	fn withdraw_collateral() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: Nomination NominationEnabled (r:0 w:1)
	/// Proof: Nomination NominationEnabled (max_values: Some(1), max_size: Some(1), added: 496, mode: MaxEncodedLen)
	fn set_nomination_enabled() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_000_000 picoseconds.
		Weight::from_parts(5_000_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Nomination NominationEnabled (r:1 w:0)
	/// Proof: Nomination NominationEnabled (max_values: Some(1), max_size: Some(1), added: 496, mode: MaxEncodedLen)
	/// Storage: VaultRegistry Vaults (r:1 w:0)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: Nomination Vaults (r:1 w:1)
	/// Proof: Nomination Vaults (max_values: None, max_size: Some(141), added: 2616, mode: MaxEncodedLen)
	fn opt_in_to_nomination() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `539`
		//  Estimated: `11120`
		// Minimum execution time: 25_000_000 picoseconds.
		Weight::from_parts(26_000_000, 0)
			.saturating_add(Weight::from_parts(0, 11120))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Nomination Vaults (r:1 w:1)
	/// Proof: Nomination Vaults (max_values: None, max_size: Some(141), added: 2616, mode: MaxEncodedLen)
	/// Storage: VaultStaking Nonce (r:1 w:1)
	/// Proof Skipped: VaultStaking Nonce (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalCurrentStake (r:2 w:2)
	/// Proof Skipped: VaultStaking TotalCurrentStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking Stake (r:2 w:2)
	/// Proof Skipped: VaultStaking Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashPerToken (r:2 w:0)
	/// Proof Skipped: VaultStaking SlashPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashTally (r:2 w:2)
	/// Proof Skipped: VaultStaking SlashTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry Vaults (r:1 w:0)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	/// Proof Skipped: VaultRegistry SecureCollateralThreshold (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalStake (r:2 w:2)
	/// Proof Skipped: VaultStaking TotalStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardTally (r:4 w:4)
	/// Proof Skipped: VaultStaking RewardTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardPerToken (r:4 w:0)
	/// Proof Skipped: VaultStaking RewardPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	/// Proof Skipped: VaultRegistry TotalUserVaultCollateral (max_values: None, max_size: None, mode: Measured)
	fn opt_out_of_nomination() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2080`
		//  Estimated: `95391`
		// Minimum execution time: 200_000_000 picoseconds.
		Weight::from_parts(203_000_000, 0)
			.saturating_add(Weight::from_parts(0, 95391))
			.saturating_add(T::DbWeight::get().reads(24))
			.saturating_add(T::DbWeight::get().writes(15))
	}
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Nomination NominationEnabled (r:1 w:0)
	/// Proof: Nomination NominationEnabled (max_values: Some(1), max_size: Some(1), added: 496, mode: MaxEncodedLen)
	/// Storage: Nomination Vaults (r:1 w:0)
	/// Proof: Nomination Vaults (max_values: None, max_size: Some(141), added: 2616, mode: MaxEncodedLen)
	/// Storage: VaultStaking Nonce (r:1 w:0)
	/// Proof Skipped: VaultStaking Nonce (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalCurrentStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking Stake (r:2 w:1)
	/// Proof Skipped: VaultStaking Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashPerToken (r:1 w:0)
	/// Proof Skipped: VaultStaking SlashPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashTally (r:2 w:1)
	/// Proof Skipped: VaultStaking SlashTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	/// Proof Skipped: VaultRegistry SecureCollateralThreshold (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry PremiumRedeemThreshold (r:1 w:0)
	/// Proof Skipped: VaultRegistry PremiumRedeemThreshold (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards Stake (r:1 w:0)
	/// Proof Skipped: VaultRewards Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards RewardPerToken (r:2 w:0)
	/// Proof Skipped: VaultRewards RewardPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards RewardTally (r:2 w:2)
	/// Proof Skipped: VaultRewards RewardTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards TotalRewards (r:2 w:2)
	/// Proof Skipped: VaultRewards TotalRewards (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardPerToken (r:2 w:2)
	/// Proof Skipped: VaultStaking RewardPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalStake (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardTally (r:2 w:2)
	/// Proof Skipped: VaultStaking RewardTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(150), added: 2625, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	/// Proof Skipped: VaultRegistry TotalUserVaultCollateral (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry SystemCollateralCeiling (r:1 w:0)
	/// Proof Skipped: VaultRegistry SystemCollateralCeiling (max_values: None, max_size: None, mode: Measured)
	fn deposit_collateral() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2955`
		//  Estimated: `139410`
		// Minimum execution time: 220_000_000 picoseconds.
		Weight::from_parts(227_000_000, 0)
			.saturating_add(Weight::from_parts(0, 139410))
			.saturating_add(T::DbWeight::get().reads(29))
			.saturating_add(T::DbWeight::get().writes(15))
	}
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: VaultStaking Nonce (r:1 w:0)
	/// Proof Skipped: VaultStaking Nonce (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry Vaults (r:1 w:0)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalCurrentStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	/// Proof Skipped: VaultRegistry SecureCollateralThreshold (max_values: None, max_size: None, mode: Measured)
	/// Storage: Nomination NominationEnabled (r:1 w:0)
	/// Proof: Nomination NominationEnabled (max_values: Some(1), max_size: Some(1), added: 496, mode: MaxEncodedLen)
	/// Storage: Nomination Vaults (r:1 w:0)
	/// Proof: Nomination Vaults (max_values: None, max_size: Some(141), added: 2616, mode: MaxEncodedLen)
	/// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	/// Proof Skipped: VaultRegistry TotalUserVaultCollateral (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards Stake (r:1 w:0)
	/// Proof Skipped: VaultRewards Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards RewardPerToken (r:2 w:0)
	/// Proof Skipped: VaultRewards RewardPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards RewardTally (r:2 w:2)
	/// Proof Skipped: VaultRewards RewardTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards TotalRewards (r:2 w:2)
	/// Proof Skipped: VaultRewards TotalRewards (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardPerToken (r:2 w:2)
	/// Proof Skipped: VaultStaking RewardPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking Stake (r:1 w:1)
	/// Proof Skipped: VaultStaking Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashPerToken (r:1 w:0)
	/// Proof Skipped: VaultStaking SlashPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashTally (r:1 w:1)
	/// Proof Skipped: VaultStaking SlashTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalStake (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardTally (r:2 w:2)
	/// Proof Skipped: VaultStaking RewardTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(150), added: 2625, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn withdraw_collateral() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3935`
		//  Estimated: `143720`
		// Minimum execution time: 223_000_000 picoseconds.
		Weight::from_parts(226_000_000, 0)
			.saturating_add(Weight::from_parts(0, 143720))
			.saturating_add(T::DbWeight::get().reads(26))
			.saturating_add(T::DbWeight::get().writes(15))
	}
}
