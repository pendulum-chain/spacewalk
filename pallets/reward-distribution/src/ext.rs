#[cfg(test)]
use mocktopus::macros::mockable;

#[cfg_attr(test, mockable)]
pub(crate) mod security {
	use frame_support::{dispatch::DispatchResult, sp_runtime::DispatchError};
	use frame_system::pallet_prelude::BlockNumberFor;

	pub fn ensure_parachain_status_running<T: crate::Config>() -> DispatchResult {
		<security::Pallet<T>>::ensure_parachain_status_running()
	}

	pub fn parachain_block_expired<T: crate::Config>(
		opentime: BlockNumberFor<T>,
		period: BlockNumberFor<T>,
	) -> Result<bool, DispatchError> {
		<security::Pallet<T>>::parachain_block_expired(opentime, period)
	}

	pub fn get_active_block<T: crate::Config>() -> BlockNumberFor<T> {
		<security::Pallet<T>>::active_block_number()
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod pooled_rewards {
	use crate::{types::BalanceOf, DefaultVaultId};
	use currency::CurrencyId;
	use frame_support::{pallet_prelude::DispatchResult, sp_runtime::DispatchError};
	use pooled_rewards::RewardsApi;
	use sp_std::vec::Vec;
	type Inner<T> = (CurrencyId<T>, BalanceOf<T>);
	pub fn get_total_stake_all_pools<T: crate::Config>() -> Result<Vec<Inner<T>>, DispatchError> {
		T::VaultRewards::get_total_stake_all_pools()
	}

	pub fn distribute_reward<T: crate::Config>(
		pool_id: &CurrencyId<T>,
		reward_currency: &CurrencyId<T>,
		amount: BalanceOf<T>,
	) -> DispatchResult {
		T::VaultRewards::distribute_reward(pool_id, *reward_currency, amount)
	}

	pub fn withdraw_reward<T: crate::Config>(
		pool_id: &CurrencyId<T>,
		vault_id: &DefaultVaultId<T>,
		reward_currency_id: CurrencyId<T>,
	) -> Result<BalanceOf<T>, DispatchError> {
		T::VaultRewards::withdraw_reward(pool_id, vault_id, reward_currency_id)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod staking {
	use crate::{types::BalanceOf, DefaultVaultId};
	use currency::CurrencyId;
	use frame_support::sp_runtime::DispatchError;
	use sp_std::vec::Vec;
	use staking::Staking;

	pub fn distribute_reward<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: BalanceOf<T>,
		currency_id: CurrencyId<T>,
	) -> Result<BalanceOf<T>, DispatchError> {
		T::VaultStaking::distribute_reward(vault_id, amount, currency_id)
	}

	pub fn withdraw_reward<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		index: Option<T::Nonce>,
		currency_id: CurrencyId<T>,
	) -> Result<BalanceOf<T>, DispatchError> {
		T::VaultStaking::withdraw_reward(vault_id, nominator_id, index, currency_id)
	}

	pub fn compute_reward<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		currency_id: CurrencyId<T>,
	) -> Result<BalanceOf<T>, DispatchError> {
		T::VaultStaking::compute_reward(vault_id, nominator_id, currency_id)
	}

	pub fn get_all_reward_currencies<T: crate::Config>() -> Result<Vec<CurrencyId<T>>, DispatchError>
	{
		T::VaultStaking::get_all_reward_currencies()
	}
}
