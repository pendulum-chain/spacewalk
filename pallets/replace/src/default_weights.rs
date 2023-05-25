
//! Autogenerated weights for `replace`
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

/// Weight functions for `replace`.
pub trait WeightInfo {
	fn request_replace() -> Weight;
	fn withdraw_replace() -> Weight;
	fn accept_replace() -> Weight;
	fn execute_replace() -> Weight;
	fn cancel_replace() -> Weight;
	fn set_replace_period() -> Weight;
	fn minimum_transfer_amount_update() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: VaultRegistry Vaults (r:1 w:1)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: Nomination Vaults (r:1 w:0)
	/// Proof: Nomination Vaults (max_values: None, max_size: Some(141), added: 2616, mode: MaxEncodedLen)
	/// Storage: Replace ReplaceMinimumTransferAmount (r:1 w:0)
	/// Proof: Replace ReplaceMinimumTransferAmount (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Fee ReplaceGriefingCollateral (r:1 w:0)
	/// Proof Skipped: Fee ReplaceGriefingCollateral (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:1 w:1)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(150), added: 2625, mode: MaxEncodedLen)
	fn request_replace() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1501`
		//  Estimated: `19660`
		// Minimum execution time: 68_000_000 picoseconds.
		Weight::from_parts(68_000_000, 0)
			.saturating_add(Weight::from_parts(0, 19660))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: VaultRegistry Vaults (r:1 w:1)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	fn withdraw_replace() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `584`
		//  Estimated: `4049`
		// Minimum execution time: 29_000_000 picoseconds.
		Weight::from_parts(30_000_000, 0)
			.saturating_add(Weight::from_parts(0, 4049))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: VaultRegistry Vaults (r:2 w:2)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: Replace ReplaceMinimumTransferAmount (r:1 w:0)
	/// Proof: Replace ReplaceMinimumTransferAmount (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	/// Proof Skipped: VaultRegistry TotalUserVaultCollateral (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry SystemCollateralCeiling (r:1 w:0)
	/// Proof Skipped: VaultRegistry SystemCollateralCeiling (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:1 w:1)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(150), added: 2625, mode: MaxEncodedLen)
	/// Storage: VaultStaking Nonce (r:1 w:0)
	/// Proof Skipped: VaultStaking Nonce (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking Stake (r:1 w:1)
	/// Proof Skipped: VaultStaking Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashPerToken (r:1 w:0)
	/// Proof Skipped: VaultStaking SlashPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashTally (r:1 w:1)
	/// Proof Skipped: VaultStaking SlashTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalStake (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalCurrentStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardTally (r:2 w:2)
	/// Proof Skipped: VaultStaking RewardTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking RewardPerToken (r:2 w:0)
	/// Proof Skipped: VaultStaking RewardPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards Stake (r:1 w:1)
	/// Proof Skipped: VaultRewards Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards TotalStake (r:1 w:1)
	/// Proof Skipped: VaultRewards TotalStake (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: VaultRewards RewardTally (r:2 w:2)
	/// Proof Skipped: VaultRewards RewardTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards RewardPerToken (r:2 w:0)
	/// Proof Skipped: VaultRewards RewardPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	/// Proof Skipped: VaultRegistry SecureCollateralThreshold (max_values: None, max_size: None, mode: Measured)
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Security Nonce (r:1 w:1)
	/// Proof Skipped: Security Nonce (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: System ParentHash (r:1 w:0)
	/// Proof: System ParentHash (max_values: Some(1), max_size: Some(32), added: 527, mode: MaxEncodedLen)
	/// Storage: Security ActiveBlockCount (r:1 w:0)
	/// Proof Skipped: Security ActiveBlockCount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Replace ReplacePeriod (r:1 w:0)
	/// Proof: Replace ReplacePeriod (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: Replace ReplaceRequests (r:0 w:1)
	/// Proof: Replace ReplaceRequests (max_values: None, max_size: Some(431), added: 2906, mode: MaxEncodedLen)
	fn accept_replace() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3734`
		//  Estimated: `149358`
		// Minimum execution time: 230_000_000 picoseconds.
		Weight::from_parts(235_000_000, 0)
			.saturating_add(Weight::from_parts(0, 149358))
			.saturating_add(T::DbWeight::get().reads(28))
			.saturating_add(T::DbWeight::get().writes(16))
	}
	/// Storage: Replace ReplaceRequests (r:1 w:1)
	/// Proof: Replace ReplaceRequests (max_values: None, max_size: Some(431), added: 2906, mode: MaxEncodedLen)
	/// Storage: StellarRelay NewValidatorsEnactmentBlockHeight (r:1 w:0)
	/// Proof: StellarRelay NewValidatorsEnactmentBlockHeight (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: StellarRelay Validators (r:1 w:0)
	/// Proof: StellarRelay Validators (max_values: Some(1), max_size: Some(70382), added: 70877, mode: MaxEncodedLen)
	/// Storage: StellarRelay Organizations (r:1 w:0)
	/// Proof: StellarRelay Organizations (max_values: Some(1), max_size: Some(37232), added: 37727, mode: MaxEncodedLen)
	/// Storage: VaultRegistry Vaults (r:2 w:2)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards Stake (r:1 w:0)
	/// Proof Skipped: VaultRewards Stake (max_values: None, max_size: None, mode: Measured)
	fn execute_replace() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1663`
		//  Estimated: `128700`
		// Minimum execution time: 8_667_000_000 picoseconds.
		Weight::from_parts(8_695_000_000, 0)
			.saturating_add(Weight::from_parts(0, 128700))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: Replace ReplaceRequests (r:1 w:1)
	/// Proof: Replace ReplaceRequests (max_values: None, max_size: Some(431), added: 2906, mode: MaxEncodedLen)
	/// Storage: Replace ReplacePeriod (r:1 w:0)
	/// Proof: Replace ReplacePeriod (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: Security ActiveBlockCount (r:1 w:0)
	/// Proof Skipped: Security ActiveBlockCount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: VaultRegistry Vaults (r:2 w:2)
	/// Proof Skipped: VaultRegistry Vaults (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards Stake (r:1 w:1)
	/// Proof Skipped: VaultRewards Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards TotalStake (r:1 w:1)
	/// Proof Skipped: VaultRewards TotalStake (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: VaultRewards RewardTally (r:2 w:2)
	/// Proof Skipped: VaultRewards RewardTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRewards RewardPerToken (r:2 w:0)
	/// Proof Skipped: VaultRewards RewardPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking Nonce (r:1 w:0)
	/// Proof Skipped: VaultStaking Nonce (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	/// Proof Skipped: VaultStaking TotalCurrentStake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	/// Proof Skipped: VaultRegistry SecureCollateralThreshold (max_values: None, max_size: None, mode: Measured)
	/// Storage: Security ParachainStatus (r:1 w:0)
	/// Proof Skipped: Security ParachainStatus (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	/// Proof Skipped: VaultRegistry TotalUserVaultCollateral (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking Stake (r:1 w:1)
	/// Proof Skipped: VaultStaking Stake (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashPerToken (r:1 w:0)
	/// Proof Skipped: VaultStaking SlashPerToken (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking SlashTally (r:1 w:1)
	/// Proof Skipped: VaultStaking SlashTally (max_values: None, max_size: None, mode: Measured)
	/// Storage: VaultStaking TotalStake (r:1 w:1)
	/// Proof Skipped: VaultStaking TotalStake (max_values: None, max_size: None, mode: Measured)
	fn cancel_replace() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2966`
		//  Estimated: `103335`
		// Minimum execution time: 135_000_000 picoseconds.
		Weight::from_parts(139_000_000, 0)
			.saturating_add(Weight::from_parts(0, 103335))
			.saturating_add(T::DbWeight::get().reads(20))
			.saturating_add(T::DbWeight::get().writes(11))
	}
	/// Storage: Replace ReplacePeriod (r:0 w:1)
	/// Proof: Replace ReplacePeriod (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	fn set_replace_period() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 9_000_000 picoseconds.
		Weight::from_parts(9_000_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: Replace ReplaceMinimumTransferAmount (r:0 w:1)
	/// Proof: Replace ReplaceMinimumTransferAmount (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	fn minimum_transfer_amount_update() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 9_000_000 picoseconds.
		Weight::from_parts(10_000_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
