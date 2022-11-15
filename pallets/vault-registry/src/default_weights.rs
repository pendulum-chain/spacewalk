// This file is part of Substrate.

// Copyright (C) 2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Autogenerated weights for vault_registry
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-11-15, STEPS: `100`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: None, WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/spacewalk-standalone
// benchmark
// pallet
// --chain=dev
// --pallet=vault-registry
// --extrinsic=*
// --steps=100
// --repeat=10
// --wasm-execution=compiled
// --output=pallets/vault-registry/src/default_weights.rs
// --template=.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for vault_registry.
pub trait WeightInfo {
	fn register_vault() -> Weight;
	fn deposit_collateral() -> Weight;
	fn withdraw_collateral() -> Weight;
	fn register_public_key() -> Weight;
	fn accept_new_issues() -> Weight;
	fn set_custom_secure_threshold() -> Weight;
	fn set_minimum_collateral() -> Weight;
	fn set_system_collateral_ceiling() -> Weight;
	fn set_secure_collateral_threshold() -> Weight;
	fn set_premium_redeem_threshold() -> Weight;
	fn set_liquidation_collateral_threshold() -> Weight;
	fn report_undercollateralized_vault() -> Weight;
	fn recover_vault_id() -> Weight;
}

/// Weights for vault_registry using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: VaultRegistry PremiumRedeemThreshold (r:1 w:0)
	// Storage: VaultRegistry LiquidationCollateralThreshold (r:1 w:0)
	// Storage: VaultRegistry MinimumCollateralVault (r:1 w:0)
	// Storage: VaultRegistry SystemCollateralCeiling (r:1 w:0)
	// Storage: VaultRegistry VaultStellarPublicKey (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	// Storage: VaultStaking RewardTally (r:2 w:2)
	// Storage: VaultStaking RewardPerToken (r:2 w:0)
	fn register_vault() -> Weight {
		(131_624_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(19 as Weight))
			.saturating_add(T::DbWeight::get().writes(9 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:0)
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
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	fn deposit_collateral() -> Weight {
		(239_747_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(17 as Weight))
			.saturating_add(T::DbWeight::get().writes(8 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultRegistry PremiumRedeemThreshold (r:1 w:0)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	// Storage: VaultStaking RewardTally (r:2 w:2)
	// Storage: VaultStaking RewardPerToken (r:2 w:0)
	fn withdraw_collateral() -> Weight {
		(240_137_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(17 as Weight))
			.saturating_add(T::DbWeight::get().writes(8 as Weight))
	}
	// Storage: VaultRegistry VaultStellarPublicKey (r:1 w:1)
	fn register_public_key() -> Weight {
		(30_224_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn accept_new_issues() -> Weight {
		(18_225_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn set_custom_secure_threshold() -> Weight {
		(22_986_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry MinimumCollateralVault (r:0 w:1)
	fn set_minimum_collateral() -> Weight {
		(5_743_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry SystemCollateralCeiling (r:0 w:1)
	fn set_system_collateral_ceiling() -> Weight {
		(6_866_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry SecureCollateralThreshold (r:0 w:1)
	fn set_secure_collateral_threshold() -> Weight {
		(6_303_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry PremiumRedeemThreshold (r:0 w:1)
	fn set_premium_redeem_threshold() -> Weight {
		(6_055_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry LiquidationCollateralThreshold (r:0 w:1)
	fn set_liquidation_collateral_threshold() -> Weight {
		(6_470_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: VaultRegistry LiquidationCollateralThreshold (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	// Storage: VaultStaking RewardTally (r:2 w:2)
	// Storage: VaultStaking RewardPerToken (r:2 w:0)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: Tokens Accounts (r:2 w:2)
	// Storage: System Account (r:2 w:1)
	// Storage: VaultRegistry SystemCollateralCeiling (r:1 w:0)
	// Storage: VaultRegistry LiquidationVault (r:1 w:1)
	// Storage: VaultRewards Stake (r:1 w:1)
	// Storage: VaultRewards TotalStake (r:1 w:1)
	// Storage: VaultRewards RewardTally (r:2 w:2)
	// Storage: VaultRewards RewardPerToken (r:2 w:0)
	fn report_undercollateralized_vault() -> Weight {
		(277_832_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(27 as Weight))
			.saturating_add(T::DbWeight::get().writes(16 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn recover_vault_id() -> Weight {
		(21_211_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: VaultRegistry PremiumRedeemThreshold (r:1 w:0)
	// Storage: VaultRegistry LiquidationCollateralThreshold (r:1 w:0)
	// Storage: VaultRegistry MinimumCollateralVault (r:1 w:0)
	// Storage: VaultRegistry SystemCollateralCeiling (r:1 w:0)
	// Storage: VaultRegistry VaultStellarPublicKey (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	// Storage: VaultStaking RewardTally (r:2 w:2)
	// Storage: VaultStaking RewardPerToken (r:2 w:0)
	fn register_vault() -> Weight {
		(131_624_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(19 as Weight))
			.saturating_add(RocksDbWeight::get().writes(9 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:0)
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
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	fn deposit_collateral() -> Weight {
		(239_747_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(17 as Weight))
			.saturating_add(RocksDbWeight::get().writes(8 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultRegistry PremiumRedeemThreshold (r:1 w:0)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	// Storage: VaultStaking RewardTally (r:2 w:2)
	// Storage: VaultStaking RewardPerToken (r:2 w:0)
	fn withdraw_collateral() -> Weight {
		(240_137_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(17 as Weight))
			.saturating_add(RocksDbWeight::get().writes(8 as Weight))
	}
	// Storage: VaultRegistry VaultStellarPublicKey (r:1 w:1)
	fn register_public_key() -> Weight {
		(30_224_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn accept_new_issues() -> Weight {
		(18_225_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn set_custom_secure_threshold() -> Weight {
		(22_986_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(2 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry MinimumCollateralVault (r:0 w:1)
	fn set_minimum_collateral() -> Weight {
		(5_743_000 as Weight)
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry SystemCollateralCeiling (r:0 w:1)
	fn set_system_collateral_ceiling() -> Weight {
		(6_866_000 as Weight)
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry SecureCollateralThreshold (r:0 w:1)
	fn set_secure_collateral_threshold() -> Weight {
		(6_303_000 as Weight)
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry PremiumRedeemThreshold (r:0 w:1)
	fn set_premium_redeem_threshold() -> Weight {
		(6_055_000 as Weight)
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry LiquidationCollateralThreshold (r:0 w:1)
	fn set_liquidation_collateral_threshold() -> Weight {
		(6_470_000 as Weight)
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: VaultRegistry LiquidationCollateralThreshold (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:1)
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: VaultStaking Stake (r:1 w:1)
	// Storage: VaultStaking SlashPerToken (r:1 w:0)
	// Storage: VaultStaking SlashTally (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:1)
	// Storage: VaultStaking RewardTally (r:2 w:2)
	// Storage: VaultStaking RewardPerToken (r:2 w:0)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: Tokens Accounts (r:2 w:2)
	// Storage: System Account (r:2 w:1)
	// Storage: VaultRegistry SystemCollateralCeiling (r:1 w:0)
	// Storage: VaultRegistry LiquidationVault (r:1 w:1)
	// Storage: VaultRewards Stake (r:1 w:1)
	// Storage: VaultRewards TotalStake (r:1 w:1)
	// Storage: VaultRewards RewardTally (r:2 w:2)
	// Storage: VaultRewards RewardPerToken (r:2 w:0)
	fn report_undercollateralized_vault() -> Weight {
		(277_832_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(27 as Weight))
			.saturating_add(RocksDbWeight::get().writes(16 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn recover_vault_id() -> Weight {
		(21_211_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
}