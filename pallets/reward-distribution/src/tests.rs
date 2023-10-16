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

		assert_ok!(RewardDistribution::distribute_rewards(100, XCM(18)));
	});
}

#[cfg(test)]
#[test]
fn on_initialize_hook_distribution_works() {
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
				assert_eq!(reward_currency, &<Test as pallet::Config>::GetNativeCurrencyId::get());
				MockResult::Return(Ok(()))
			},
		);

		System::set_block_number(100);
		let new_rewards_per_block = 100;
		assert_ok!(RewardDistribution::set_reward_per_block(
			RuntimeOrigin::root(),
			new_rewards_per_block
		));

		assert_eq!(RewardDistribution::rewards_adapted_at(), Some(100));

		System::set_block_number(101);
		RewardDistribution::execute_on_init(101);
	})
}

#[cfg(test)]
#[test]
fn udpate_reward_does_not_trigger_incorrectly() {
	run_test(|| {
		ext::pooled_rewards::get_total_stake_all_pools::<Test>.mock_safe(move || {
			let initial_stakes = build_total_stakes();
			MockResult::Return(Ok(initial_stakes))
		});

		let new_rewards_per_block = 100;
		assert_ok!(RewardDistribution::set_reward_per_block(
			RuntimeOrigin::root(),
			new_rewards_per_block
		));

		System::set_block_number(2);
		RewardDistribution::execute_on_init(2);

		//same reward per block is expected
		assert_eq!(pallet::RewardPerBlock::<Test>::get(), Some(new_rewards_per_block));
	})
}

#[cfg(test)]
#[test]
fn udpate_reward_per_block_works() {
	run_test(|| {
		ext::pooled_rewards::get_total_stake_all_pools::<Test>.mock_safe(move || {
			let initial_stakes = build_total_stakes();
			MockResult::Return(Ok(initial_stakes))
		});
		ext::security::parachain_block_expired::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(true)));

		let new_rewards_per_block = 10000;
		assert_ok!(RewardDistribution::set_reward_per_block(
			RuntimeOrigin::root(),
			new_rewards_per_block
		));

		System::set_block_number(200);
		//Security::<Test>::set_active_block_number(200);
		RewardDistribution::execute_on_init(200);

		//we expect rewards to have fallen by DecayRate percent from initial
		assert_eq!(pallet::RewardPerBlock::<Test>::get(), Some(9500));

		System::set_block_number(300);
		//Security::<Test>::set_active_block_number(300);
		RewardDistribution::execute_on_init(300);
		//we expect reward to have fallen by DecayRate twice (compound)
		assert_eq!(pallet::RewardPerBlock::<Test>::get(), Some(9025));
	})
}
