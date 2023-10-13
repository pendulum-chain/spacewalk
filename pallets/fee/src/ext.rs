#[cfg(test)]
use mocktopus::macros::mockable;

#[cfg_attr(test, mockable)]
pub(crate) mod reward_distribution {
	use crate::DispatchError;
	use currency::Amount;
	use reward_distribution::DistributeRewardsToPool;

	pub fn distribute_rewards<T: crate::Config>(
		reward: &Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		let undistributed_balance =
			T::DistributePool::distribute_rewards(reward.amount(), reward.currency())?;
		Ok(Amount::<T>::new(undistributed_balance, reward.currency()))
	}
}
