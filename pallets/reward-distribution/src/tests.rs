use crate::mock::*;
use frame_support::{assert_err, assert_ok};
use frame_system::Origin;
use sp_runtime::DispatchError::BadOrigin;

#[cfg(test)]
#[test]
fn test_set_rewards_per_block() {
	run_test(|| {
		let new_rewards_per_block = 100;

		assert_err!(
			RewardDistribution::set_rewards_per_block(Origin::signed(1), new_rewards_per_block),
			BadOrigin
		);

		assert_err!(
			RewardDistribution::set_rewards_per_block(Origin::none(), new_rewards_per_block),
			BadOrigin
		);

		assert_ok!(RewardDistribution::set_rewards_per_block(
			Origin::root(),
			new_rewards_per_block
		));

		assert_eq!(RewardDistribution::rewards_per_block(), new_rewards_per_block);
	});
}
