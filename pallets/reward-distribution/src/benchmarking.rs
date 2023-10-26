#[allow(unused)]
use super::Pallet as RewardDistribution;

use super::*;
use crate::types::DefaultVaultId;
use currency::getters::get_relay_chain_currency_id as get_collateral_currency_id;
pub use currency::testing_constants::{DEFAULT_COLLATERAL_CURRENCY, DEFAULT_WRAPPED_CURRENCY};
use frame_benchmarking::{
	v2::{account, benchmarks, impl_benchmark_test_suite},
	vec,
};
use frame_system::RawOrigin;
use pooled_rewards::RewardsApi;
use staking::Staking;
#[benchmarks]
pub mod benchmarks {
	use super::*;

	#[benchmark]
	fn set_reward_per_block() {
		let new_reward_per_block = Default::default();

		#[extrinsic_call]
		_(RawOrigin::Root, new_reward_per_block);

		assert_eq!(RewardDistribution::<T>::reward_per_block(), Some(new_reward_per_block));
	}

	#[benchmark]
	fn collect_reward() {
		//initial values
		let collateral_currency = get_collateral_currency_id::<T>();
		let native_currency_id = T::GetNativeCurrencyId::get();
		let nominator = account("Alice", 0, 0);
		let nominated_amount: u32 = 40000;
		let reward_to_distribute: u32 = 100;
		NativeLiability::<T>::set(Some(10000u64.into()));
		//get the vault
		let vault_id: DefaultVaultId<T> = DefaultVaultId::<T>::new(
			account("Vault", 0, 0),
			collateral_currency,
			collateral_currency,
		);

		let _stake = T::VaultStaking::deposit_stake(&vault_id, &nominator, nominated_amount.into())
			.expect("error at deposit stake");
		let _reward_stake = T::VaultRewards::deposit_stake(
			&collateral_currency,
			&vault_id,
			nominated_amount.into(),
		)
		.expect("error at deposit stake into pool rewards");
		let _distributed = T::VaultRewards::distribute_reward(
			&collateral_currency,
			native_currency_id.clone(),
			reward_to_distribute.clone().into(),
		)
		.expect("error at distribute rewards");

		#[extrinsic_call]
		_(RawOrigin::Signed(nominator.clone()), vault_id, native_currency_id, None);
	}

	#[benchmark]
	fn on_initialize() {
		//initial values
		let collateral_currency = get_collateral_currency_id::<T>();
		let _native_currency_id = T::GetNativeCurrencyId::get();
		let nominator = account("Alice", 0, 0);
		let nominated_amount: u32 = 40000;

		//set reward per block
		let new_reward_per_block: u64 = 5;
		RewardDistribution::<T>::set_reward_per_block(
			RawOrigin::Root.into(),
			new_reward_per_block.clone().into(),
		)
		.expect("Could no set reward per block");
		assert_eq!(RewardDistribution::<T>::reward_per_block(), Some(new_reward_per_block.into()));

		//set the vault and nominate it
		let vault_id: DefaultVaultId<T> = DefaultVaultId::<T>::new(
			account("Vault", 0, 0),
			collateral_currency,
			collateral_currency,
		);

		let _stake = T::VaultStaking::deposit_stake(&vault_id, &nominator, nominated_amount.into())
			.expect("error at deposit stake");
		let _reward_stake = T::VaultRewards::deposit_stake(
			&collateral_currency,
			&vault_id,
			nominated_amount.into(),
		)
		.expect("error at deposit stake into pool rewards");

		// `on_initialize` benchmark call
		#[block]
		{
			RewardDistribution::<T>::execute_on_init(1u32.into());
		}
	}

	impl_benchmark_test_suite!(
		RewardDistribution,
		crate::mock::ExtBuilder::build(),
		crate::mock::Test
	);
}
