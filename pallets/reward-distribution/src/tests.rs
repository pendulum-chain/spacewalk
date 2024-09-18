use crate::{ext, mock::*, pallet, DefaultVaultId, Error, NativeLiability};
pub use currency::testing_constants::{DEFAULT_COLLATERAL_CURRENCY, DEFAULT_WRAPPED_CURRENCY};
use frame_benchmarking::account;
use frame_support::{assert_err, assert_ok, traits::Get};
use frame_system::pallet_prelude::BlockNumberFor;
use mocktopus::mocking::*;
use oracle::OracleApi;
use primitives::CurrencyId::XCM;
use frame_support::sp_runtime::{traits::One, DispatchError::BadOrigin};
use staking::Staking;
const COLLATERAL_POOL_1: u128 = 1000u128;
const COLLATERAL_POOL_2: u128 = 3000u128;
const COLLATERAL_POOL_3: u128 = 5000u128;
const COLLATERAL_POOL_4: u128 = 500u128;

fn build_total_stakes(
) -> Vec<(<Test as orml_tokens::Config>::CurrencyId, <Test as pallet::Config>::Balance)> {
	//total in usd 215000
	vec![
		(DEFAULT_COLLATERAL_CURRENCY, COLLATERAL_POOL_1),
		(XCM(1), COLLATERAL_POOL_2),
		(XCM(2), COLLATERAL_POOL_3),
		(XCM(3), COLLATERAL_POOL_4),
	]
}

fn to_usd(
	amount: &<Test as pallet::Config>::Balance,
	currency_id: &<Test as orml_tokens::Config>::CurrencyId,
) -> <Test as pallet::Config>::Balance {
	OracleApiMock::currency_to_usd(amount, currency_id).expect("currency not found in oracle mock")
}

fn expected_vault_rewards(
	reward: <Test as pallet::Config>::Balance,
) -> Vec<<Test as pallet::Config>::Balance> {
	let total_collateral_in_usd = to_usd(&COLLATERAL_POOL_1, &DEFAULT_COLLATERAL_CURRENCY) +
		to_usd(&COLLATERAL_POOL_2, &XCM(1)) +
		to_usd(&COLLATERAL_POOL_3, &XCM(2)) +
		to_usd(&COLLATERAL_POOL_4, &XCM(3));

	let reward_pool_1 = (reward as f64 *
		(to_usd(&COLLATERAL_POOL_1, &DEFAULT_COLLATERAL_CURRENCY) as f64) /
		(total_collateral_in_usd as f64)) as u128;
	let reward_pool_2 = (reward as f64 * (to_usd(&COLLATERAL_POOL_2, &XCM(1)) as f64) /
		(total_collateral_in_usd as f64)) as u128;
	let reward_pool_3 = (reward as f64 * (to_usd(&COLLATERAL_POOL_3, &XCM(2)) as f64) /
		(total_collateral_in_usd as f64)) as u128;
	let reward_pool_4 = (reward as f64 * (to_usd(&COLLATERAL_POOL_4, &XCM(3)) as f64) /
		(total_collateral_in_usd as f64)) as u128;

	vec![reward_pool_1, reward_pool_2, reward_pool_3, reward_pool_4]
}

#[cfg(test)]
#[test]
fn test_set_rewards_per_block() {
	run_test(|| {
		let new_rewards_per_block = 100;

		ext::security::get_active_block::<Test>.mock_safe(move || MockResult::Return(1));

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
		let reward = 100u128;
		ext::pooled_rewards::get_total_stake_all_pools::<Test>.mock_safe(move || {
			let initial_stakes = build_total_stakes();
			MockResult::Return(Ok(initial_stakes))
		});

		let mut expected_pool_ids = vec![XCM(0), XCM(1), XCM(2), XCM(3)].into_iter();
		let mut expected_stake_per_pool = expected_vault_rewards(reward).into_iter();
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

		assert_ok!(RewardDistribution::distribute_rewards(reward, XCM(18)));
	});
}

#[cfg(test)]
#[test]
fn on_initialize_hook_distribution_works() {
	run_test(|| {
		let new_rewards_per_block = 100;
		ext::pooled_rewards::get_total_stake_all_pools::<Test>.mock_safe(move || {
			let initial_stakes = build_total_stakes();
			MockResult::Return(Ok(initial_stakes))
		});
		ext::security::get_active_block::<Test>.mock_safe(move || MockResult::Return(1));

		let mut expected_pool_ids = vec![XCM(0), XCM(1), XCM(2), XCM(3)].into_iter();
		let mut expected_stake_per_pool = expected_vault_rewards(new_rewards_per_block).into_iter();
		ext::pooled_rewards::distribute_reward::<Test>.mock_safe(
			move |pool_id, reward_currency, amount| {
				let expected_pool_id = expected_pool_ids.next().expect("More calls than expected");
				let expected_stake_per_this_pool =
					expected_stake_per_pool.next().expect("More calls than expected");
				assert_eq!(pool_id, &expected_pool_id);
				assert_eq!(amount, expected_stake_per_this_pool);
				assert_eq!(
					reward_currency,
					&<Test as orml_currencies::Config>::GetNativeCurrencyId::get()
				);
				MockResult::Return(Ok(()))
			},
		);
		let block_number: BlockNumberFor<Test> = <Test as pallet::Config>::DecayInterval::get();

		System::set_block_number(block_number);
		ext::security::get_active_block::<Test>.mock_safe(move || MockResult::Return(block_number));

		assert_ok!(RewardDistribution::set_reward_per_block(
			RuntimeOrigin::root(),
			new_rewards_per_block
		));

		assert_eq!(RewardDistribution::rewards_adapted_at(), Some(100));

		let new_block_number = block_number + BlockNumberFor::<Test>::one();
		System::set_block_number(new_block_number);
		ext::security::get_active_block::<Test>
			.mock_safe(move || MockResult::Return(new_block_number));
		RewardDistribution::execute_on_init(new_block_number);
		assert_eq!(RewardDistribution::native_liability(), Some(new_rewards_per_block));
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

		ext::security::parachain_block_expired::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(false)));

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
		let block_number: BlockNumberFor<Test> = <Test as pallet::Config>::DecayInterval::get();

		ext::pooled_rewards::get_total_stake_all_pools::<Test>.mock_safe(move || {
			let initial_stakes = build_total_stakes();
			MockResult::Return(Ok(initial_stakes))
		});

		//NOTE: setting this to always return true is sufficient to trigger the reward update
		// changing the block numbers on this tests MAY NOT affect the execution
		ext::security::parachain_block_expired::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(true)));

		ext::security::get_active_block::<Test>.mock_safe(move || MockResult::Return(block_number));

		let new_rewards_per_block = 10000;
		assert_ok!(RewardDistribution::set_reward_per_block(
			RuntimeOrigin::root(),
			new_rewards_per_block
		));

		System::set_block_number(block_number);
		RewardDistribution::execute_on_init(block_number);

		//we expect rewards to have fallen by DecayRate percent from initial
		assert_eq!(pallet::RewardPerBlock::<Test>::get(), Some(9500));

		let next_block_number = block_number + block_number;

		System::set_block_number(next_block_number);
		ext::security::get_active_block::<Test>
			.mock_safe(move || MockResult::Return(next_block_number));
		RewardDistribution::execute_on_init(next_block_number);
		//we expect reward to have fallen by DecayRate twice (compound)
		assert_eq!(pallet::RewardPerBlock::<Test>::get(), Some(9025));
	})
}

#[cfg(test)]
#[test]
fn collect_rewards_works_native() {
	//initial values
	run_test(|| {
		use pooled_rewards::RewardsApi;
		let collateral_currency = XCM(1);
		let native_currency_id = <Test as orml_currencies::Config>::GetNativeCurrencyId::get();
		let nominator = account("Alice", 0, 0);
		let nominated_amount: u128 = 40000;
		let rewards_per_block: u128 = 10000;
		//set reward per block
		assert_ok!(RewardDistribution::set_reward_per_block(
			RuntimeOrigin::root(),
			rewards_per_block
		));

		//get the vault
		let vault_id: DefaultVaultId<Test> = DefaultVaultId::<Test>::new(
			account("Vault", 0, 0),
			collateral_currency,
			collateral_currency,
		);

		assert_ok!(
			<<Test as pallet::Config>::VaultStaking as Staking<_, _, _, _, _>>::deposit_stake(
				&vault_id,
				&nominator,
				nominated_amount
			)
		);
		assert_ok!(<<Test as pallet::Config>::VaultRewards as RewardsApi<_, _, _>>::deposit_stake(
			&collateral_currency,
			&vault_id,
			nominated_amount
		));
		RewardDistribution::execute_on_init(1);

		assert_eq!(NativeLiability::<Test>::get(), Some(rewards_per_block));

		assert_ok!(pallet::Pallet::<Test>::collect_reward(
			RuntimeOrigin::signed(nominator),
			vault_id,
			native_currency_id,
			None
		));

		assert_eq!(Balances::free_balance(&nominator), rewards_per_block.into());

		assert_eq!(NativeLiability::<Test>::get(), Some(0));
	})
}

#[cfg(test)]
#[test]
fn collect_rewards_works_wrapped() {
	//initial values

	use orml_traits::MultiCurrency;
	run_test(|| {
		use pooled_rewards::RewardsApi;
		let collateral_currency = XCM(1);
		let nominator = account("Alice", 0, 0);
		let nominated_amount: u128 = 40000;
		let rewards: u128 = 10000;
		let fee_account = RewardDistribution::fee_pool_account_id();
		//fund fee account with the proper rewards
		assert_ok!(<orml_tokens::Pallet<Test> as MultiCurrency<AccountId>>::deposit(
			DEFAULT_WRAPPED_CURRENCY,
			&fee_account,
			rewards
		));
		//get the vault
		let vault_id: DefaultVaultId<Test> = DefaultVaultId::<Test>::new(
			account("Vault", 0, 0),
			collateral_currency,
			DEFAULT_WRAPPED_CURRENCY,
		);

		assert_ok!(
			<<Test as pallet::Config>::VaultStaking as Staking<_, _, _, _, _>>::deposit_stake(
				&vault_id,
				&nominator,
				nominated_amount
			)
		);
		assert_ok!(<<Test as pallet::Config>::VaultRewards as RewardsApi<_, _, _>>::deposit_stake(
			&collateral_currency,
			&vault_id,
			nominated_amount
		));

		assert_ok!(RewardDistribution::distribute_rewards(rewards, DEFAULT_WRAPPED_CURRENCY));

		assert_ok!(pallet::Pallet::<Test>::collect_reward(
			RuntimeOrigin::signed(nominator),
			vault_id,
			DEFAULT_WRAPPED_CURRENCY,
			None
		));

		assert_eq!(
			<orml_tokens::Pallet<Test> as MultiCurrency<AccountId>>::free_balance(
				DEFAULT_WRAPPED_CURRENCY,
				&nominator
			),
			rewards
		);
	})
}

#[cfg(test)]
#[test]
fn collect_rewards_fails_if_no_rewards_for_user() {
	//initial values

	use frame_support::assert_noop;
	run_test(|| {
		use pooled_rewards::RewardsApi;
		let collateral_currency = XCM(1);
		let native_currency_id = <Test as orml_currencies::Config>::GetNativeCurrencyId::get();
		let nominator = account("Alice", 0, 0);
		let nominated_amount: u128 = 40000;
		//we set this to a large value for testing, this would be set
		//during distribution of rewards
		NativeLiability::<Test>::set(Some(10000u64.into()));

		//get the vault
		let vault_id: DefaultVaultId<Test> = DefaultVaultId::<Test>::new(
			account("Vault", 0, 0),
			collateral_currency,
			collateral_currency,
		);

		assert_ok!(
			<<Test as pallet::Config>::VaultStaking as Staking<_, _, _, _, _>>::deposit_stake(
				&vault_id,
				&nominator,
				nominated_amount
			)
		);
		assert_ok!(<<Test as pallet::Config>::VaultRewards as RewardsApi<_, _, _>>::deposit_stake(
			&collateral_currency,
			&vault_id,
			nominated_amount
		));

		assert_noop!(
			pallet::Pallet::<Test>::collect_reward(
				RuntimeOrigin::signed(nominator),
				vault_id,
				native_currency_id,
				None
			),
			Error::<Test>::NoRewardsForAccount
		);
	})
}
