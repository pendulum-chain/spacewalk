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

//! Autogenerated weights for redeem
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-11-15, STEPS: `100`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: None, WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/spacewalk-standalone
// benchmark
// pallet
// --chain=dev
// --pallet=redeem
// --extrinsic=*
// --steps=100
// --repeat=10
// --wasm-execution=compiled
// --output=pallets/redeem/src/default_weights.rs
// --template=.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for redeem.
pub trait WeightInfo {
	fn request_redeem() -> Weight;
	fn liquidation_redeem() -> Weight;
	fn execute_redeem() -> Weight;
	fn cancel_redeem_reimburse() -> Weight;
	fn cancel_redeem_retry() -> Weight;
	fn set_redeem_period() -> Weight;
	fn self_redeem() -> Weight;
}

/// Weights for redeem using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: Fee RedeemFee (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Redeem RedeemMinimumTransferAmount (r:1 w:0)
	// Storage: VaultRewards Stake (r:1 w:1)
	// Storage: VaultRewards TotalStake (r:1 w:1)
	// Storage: VaultRewards RewardTally (r:2 w:2)
	// Storage: VaultRewards RewardPerToken (r:2 w:0)
	// Storage: Security Nonce (r:1 w:1)
	// Storage: System ParentHash (r:1 w:0)
	// Storage: VaultRegistry PremiumRedeemThreshold (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: Fee PremiumRedeemFee (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: Redeem RedeemPeriod (r:1 w:0)
	// Storage: Redeem RedeemRequests (r:0 w:1)
	fn request_redeem() -> Weight {
		(131_535_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(20 as Weight))
			.saturating_add(T::DbWeight::get().writes(8 as Weight))
	}
	// Storage: Tokens Accounts (r:3 w:3)
	// Storage: Tokens TotalIssuance (r:1 w:1)
	// Storage: VaultRegistry LiquidationVault (r:1 w:1)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: System Account (r:2 w:1)
	fn liquidation_redeem() -> Weight {
		(109_834_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(8 as Weight))
			.saturating_add(T::DbWeight::get().writes(7 as Weight))
	}
	// Storage: Redeem RedeemRequests (r:1 w:1)
	// Storage: StellarRelay IsPublicNetwork (r:1 w:0)
	// Storage: StellarRelay Validators (r:1 w:0)
	// Storage: StellarRelay Organizations (r:1 w:0)
	// Storage: VaultRewards TotalStake (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn execute_redeem() -> Weight {
		(10_967_066_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(6 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Redeem RedeemRequests (r:1 w:1)
	// Storage: Redeem RedeemPeriod (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: Fee PunishmentFee (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:0)
	// Storage: VaultRegistry PunishmentDelay (r:1 w:0)
	// Storage: VaultRewards TotalStake (r:1 w:0)
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: VaultRewards Stake (r:1 w:0)
	fn cancel_redeem_reimburse() -> Weight {
		(121_353_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(15 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Redeem RedeemRequests (r:1 w:1)
	// Storage: Redeem RedeemPeriod (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: Fee PunishmentFee (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:0)
	// Storage: VaultRegistry PunishmentDelay (r:1 w:0)
	// Storage: VaultRewards Stake (r:1 w:0)
	fn cancel_redeem_retry() -> Weight {
		(101_857_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(13 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
	// Storage: Redeem RedeemPeriod (r:0 w:1)
	fn set_redeem_period() -> Weight {
		(14_908_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: Tokens TotalIssuance (r:1 w:1)
	// Storage: VaultRewards TotalStake (r:1 w:1)
	// Storage: VaultRewards RewardPerToken (r:2 w:1)
	// Storage: VaultRewards TotalRewards (r:1 w:1)
	// Storage: VaultRewards Stake (r:1 w:1)
	// Storage: VaultRewards RewardTally (r:2 w:2)
	fn self_redeem() -> Weight {
		(119_418_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(10 as Weight))
			.saturating_add(T::DbWeight::get().writes(9 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: Fee RedeemFee (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Redeem RedeemMinimumTransferAmount (r:1 w:0)
	// Storage: VaultRewards Stake (r:1 w:1)
	// Storage: VaultRewards TotalStake (r:1 w:1)
	// Storage: VaultRewards RewardTally (r:2 w:2)
	// Storage: VaultRewards RewardPerToken (r:2 w:0)
	// Storage: Security Nonce (r:1 w:1)
	// Storage: System ParentHash (r:1 w:0)
	// Storage: VaultRegistry PremiumRedeemThreshold (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: Fee PremiumRedeemFee (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: Redeem RedeemPeriod (r:1 w:0)
	// Storage: Redeem RedeemRequests (r:0 w:1)
	fn request_redeem() -> Weight {
		(131_535_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(20 as Weight))
			.saturating_add(RocksDbWeight::get().writes(8 as Weight))
	}
	// Storage: Tokens Accounts (r:3 w:3)
	// Storage: Tokens TotalIssuance (r:1 w:1)
	// Storage: VaultRegistry LiquidationVault (r:1 w:1)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: System Account (r:2 w:1)
	fn liquidation_redeem() -> Weight {
		(109_834_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(8 as Weight))
			.saturating_add(RocksDbWeight::get().writes(7 as Weight))
	}
	// Storage: Redeem RedeemRequests (r:1 w:1)
	// Storage: StellarRelay IsPublicNetwork (r:1 w:0)
	// Storage: StellarRelay Validators (r:1 w:0)
	// Storage: StellarRelay Organizations (r:1 w:0)
	// Storage: VaultRewards TotalStake (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn execute_redeem() -> Weight {
		(10_967_066_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(6 as Weight))
			.saturating_add(RocksDbWeight::get().writes(2 as Weight))
	}
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Redeem RedeemRequests (r:1 w:1)
	// Storage: Redeem RedeemPeriod (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: Fee PunishmentFee (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:0)
	// Storage: VaultRegistry PunishmentDelay (r:1 w:0)
	// Storage: VaultRewards TotalStake (r:1 w:0)
	// Storage: VaultRegistry SecureCollateralThreshold (r:1 w:0)
	// Storage: VaultRewards Stake (r:1 w:0)
	fn cancel_redeem_reimburse() -> Weight {
		(121_353_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(15 as Weight))
			.saturating_add(RocksDbWeight::get().writes(3 as Weight))
	}
	// Storage: Security ParachainStatus (r:1 w:0)
	// Storage: Redeem RedeemRequests (r:1 w:1)
	// Storage: Redeem RedeemPeriod (r:1 w:0)
	// Storage: Security ActiveBlockCount (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Oracle Aggregate (r:1 w:0)
	// Storage: Fee PunishmentFee (r:1 w:0)
	// Storage: VaultStaking Nonce (r:1 w:0)
	// Storage: VaultStaking TotalCurrentStake (r:1 w:0)
	// Storage: VaultRegistry TotalUserVaultCollateral (r:1 w:1)
	// Storage: VaultStaking TotalStake (r:1 w:0)
	// Storage: VaultRegistry PunishmentDelay (r:1 w:0)
	// Storage: VaultRewards Stake (r:1 w:0)
	fn cancel_redeem_retry() -> Weight {
		(101_857_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(13 as Weight))
			.saturating_add(RocksDbWeight::get().writes(3 as Weight))
	}
	// Storage: Redeem RedeemPeriod (r:0 w:1)
	fn set_redeem_period() -> Weight {
		(14_908_000 as Weight)
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: Tokens TotalIssuance (r:1 w:1)
	// Storage: VaultRewards TotalStake (r:1 w:1)
	// Storage: VaultRewards RewardPerToken (r:2 w:1)
	// Storage: VaultRewards TotalRewards (r:1 w:1)
	// Storage: VaultRewards Stake (r:1 w:1)
	// Storage: VaultRewards RewardTally (r:2 w:2)
	fn self_redeem() -> Weight {
		(119_418_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(10 as Weight))
			.saturating_add(RocksDbWeight::get().writes(9 as Weight))
	}
}