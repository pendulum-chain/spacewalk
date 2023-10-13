use crate::{ext, mock::*, pallet};
use frame_support::{assert_err, assert_ok};
use mocktopus::mocking::*;
use sp_runtime::DispatchError::BadOrigin;

pub use currency::testing_constants::DEFAULT_COLLATERAL_CURRENCY;
use primitives::CurrencyId::XCM;

fn build_total_stakes(
) -> Vec<(<Test as pallet::Config>::Currency, <Test as pallet::Config>::Balance)> {
	//total in usd 215000
	vec![(DEFAULT_COLLATERAL_CURRENCY, 1000), (XCM(1), 3000), (XCM(2), 5000), (XCM(3), 500)]
}

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

		assert_eq!(RewardDistribution::reward_per_block(), None);
		assert_eq!(RewardDistribution::rewards_adapted_at(), None);

		assert_ok!(RewardDistribution::set_reward_per_block(
			RuntimeOrigin::root(),
			new_rewards_per_block
		));

		assert_eq!(RewardDistribution::reward_per_block(), Some(new_rewards_per_block));
		assert_eq!(RewardDistribution::rewards_adapted_at(), Some(1));
	});
}

#[cfg(test)]
#[test]
fn distributes_rewards_properly() {
	run_test(|| {
		ext::pooled_rewards::get_total_stake_all_pools::<Test>.mock_safe(move || {
			let initial_stakes = build_total_stakes();
			MockResult::Return(Ok(initial_stakes))
		});

		let mut expected_pool_ids = vec![XCM(0), XCM(1), XCM(2), XCM(3)].into_iter();
		let mut expected_stake_per_pool = vec![46, 13, 34, 4].into_iter();
		ext::pooled_rewards::distribute_reward::<Test>.mock_safe(
			move |pool_id, reward_currency, amount| {
				let expected_pool_id = expected_pool_ids.next().expect("More calls than expected");
				let expected_stake_per_this_pool =
					expected_stake_per_pool.next().expect("More calls than expected");
				assert_eq!(pool_id, &expected_pool_id);
				assert_eq!(amount, expected_stake_per_this_pool);
				assert_eq!(reward_currency, &XCM(18));
				MockResult::Return(Ok(()))
			},
		);

		let _ = RewardDistribution::distribute_rewards(100, XCM(18)).unwrap();

		assert_eq!(Some(1), Some(1));
	});
}
