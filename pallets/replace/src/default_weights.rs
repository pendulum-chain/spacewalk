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

//! Autogenerated weights for replace
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-11-10, STEPS: `100`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
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
		Weight::from_ref_time(0)
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn withdraw_replace() -> Weight {
		Weight::from_ref_time(0)
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
		Weight::from_ref_time(0)
	}
	// Storage: Replace ReplaceRequests (r:1 w:1)
	// Storage: StellarRelay IsPublicNetwork (r:1 w:0)
	// Storage: StellarRelay Validators (r:1 w:0)
	// Storage: StellarRelay Organizations (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:2 w:2)
	// Storage: VaultRewards Stake (r:1 w:0)
	fn execute_replace() -> Weight {
		Weight::from_ref_time(0)
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
		Weight::from_ref_time(0)
	}
	// Storage: Replace ReplacePeriod (r:0 w:1)
	fn set_replace_period() -> Weight {
		Weight::from_ref_time(0)
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
		Weight::from_ref_time(0)
	}
	// Storage: VaultRegistry Vaults (r:1 w:1)
	fn withdraw_replace() -> Weight {
		Weight::from_ref_time(0)
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
		Weight::from_ref_time(0)
	}
	// Storage: Replace ReplaceRequests (r:1 w:1)
	// Storage: StellarRelay IsPublicNetwork (r:1 w:0)
	// Storage: StellarRelay Validators (r:1 w:0)
	// Storage: StellarRelay Organizations (r:1 w:0)
	// Storage: VaultRegistry Vaults (r:2 w:2)
	// Storage: VaultRewards Stake (r:1 w:0)
	fn execute_replace() -> Weight {
		Weight::from_ref_time(0)
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
		Weight::from_ref_time(0)
	}
	// Storage: Replace ReplacePeriod (r:0 w:1)
	fn set_replace_period() -> Weight {
		Weight::from_ref_time(0)
	}
}