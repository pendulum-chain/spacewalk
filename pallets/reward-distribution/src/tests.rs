use crate::mock::*;
use frame_support::{assert_err, assert_ok};
use sp_runtime::DispatchError::BadOrigin;

#[cfg(test)]
#[test]
fn test_set_rewards_per_block() {
	run_test(|| {
		let new_rewards_per_block = 100;

		assert_err!(
			RewardDistribution::set_reward_per_block(
				RuntimeOrigin::signed(1),
				new_rewards_per_block
			),
			BadOrigin
		);

		assert_err!(
			RewardDistribution::set_reward_per_block(RuntimeOrigin::none(), new_rewards_per_block),
			BadOrigin
		);

		assert_ok!(RewardDistribution::set_reward_per_block(
			RuntimeOrigin::root(),
			new_rewards_per_block
		));

		assert_eq!(RewardDistribution::reward_per_block(), Some(new_rewards_per_block));
	});
}
