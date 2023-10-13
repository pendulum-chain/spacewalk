#[cfg(test)]
use mocktopus::macros::mockable;

#[cfg_attr(test, mockable)]
pub(crate) mod security {
	use frame_support::dispatch::DispatchResult;
	use sp_runtime::DispatchError;

	pub fn ensure_parachain_status_running<T: crate::Config>() -> DispatchResult {
		<security::Pallet<T>>::ensure_parachain_status_running()
	}

	pub fn parachain_block_expired<T: crate::Config>(
		opentime: T::BlockNumber,
		period: T::BlockNumber,
	) -> Result<bool, DispatchError> {
		<security::Pallet<T>>::parachain_block_expired(opentime, period)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod pooled_rewards {
	use frame_support::pallet_prelude::DispatchResult;
	use pooled_rewards::RewardsApi;
	use sp_runtime::DispatchError;

	pub fn get_total_stake_all_pools<T: crate::Config>(
	) -> Result<Vec<(T::Currency, T::Balance)>, DispatchError> {
		T::VaultRewards::get_total_stake_all_pools()
	}

	pub fn distribute_reward<T: crate::Config>(
		pool_id: &T::Currency,
		reward_currency: &T::Currency,
		amount: T::Balance,
	) -> DispatchResult {
		T::VaultRewards::distribute_reward(pool_id, reward_currency.clone(), amount)
	}
}
