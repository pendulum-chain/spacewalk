use frame_support::{assert_err, assert_ok};
use rand::Rng;

/// Tests for Reward
use crate::mock::*;

// type Event = crate::Event<Test>;

macro_rules! fixed {
	($amount:expr) => {
		sp_arithmetic::FixedI128::from($amount)
	};
}

#[test]
#[cfg_attr(rustfmt, rustfmt_skip)]
fn reproduce_live_state() {
    // This function is most useful for debugging. Keeping this test here for convenience
    // and to function as an additional regression test
    run_test(|| {
        let f = |x: i128| SignedFixedPoint::from_inner(x);
        let currency = DEFAULT_NATIVE_CURRENCY;

        // state for a3eFe9M2HbAgrQrShEDH2CEvXACtzLhSf4JGkwuT9SQ1EV4ti at block 0xb47ed0e773e25c81da2cc606495ab6f716c3c2024f9beb361605860912fee652
        crate::RewardPerToken::<Test>::insert(currency, f(1_699_249_738_518_636_122_154_288_694));
        crate::RewardTally::<Test>::insert(currency, ALICE, f(164_605_943_476_265_834_062_592_062_507_811_208));
        crate::Stake::<Test>::insert(ALICE, f(97_679_889_000_000_000_000_000_000));
        crate::TotalRewards::<Test>::insert(currency, f(8_763_982_459_262_268_000_000_000_000_000_000));
        crate::TotalStake::<Test>::put(f(2_253_803_217_000_000_000_000_000_000));

        assert_ok!(Reward::compute_reward(currency, &ALICE), 1376582365513566);
    })
}

#[test]
fn should_distribute_rewards_equally() {
	run_test(|| {
		assert_ok!(Reward::deposit_stake(&ALICE, fixed!(50), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::deposit_stake(&BOB, fixed!(50), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::distribute_reward(DEFAULT_WRAPPED_CURRENCY, fixed!(100)));
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &ALICE), 50);
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &BOB), 50);
	})
}

#[test]
fn should_distribute_uneven_rewards_equally() {
	run_test(|| {
		assert_ok!(Reward::deposit_stake(&ALICE, fixed!(50), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::deposit_stake(&BOB, fixed!(50), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::distribute_reward(DEFAULT_WRAPPED_CURRENCY, fixed!(451)));
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &ALICE), 225);
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &BOB), 225);
	})
}

#[test]
fn should_not_update_previous_rewards() {
	run_test(|| {
		assert_ok!(Reward::deposit_stake(&ALICE, fixed!(40), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::distribute_reward(DEFAULT_WRAPPED_CURRENCY, fixed!(1000)));
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &ALICE), 1000);

		assert_ok!(Reward::deposit_stake(&BOB, fixed!(20), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &ALICE), 1000);
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &BOB), 0);
	})
}

#[test]
fn should_withdraw_reward() {
	run_test(|| {
		assert_ok!(Reward::deposit_stake(&ALICE, fixed!(45), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::deposit_stake(&BOB, fixed!(55), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::distribute_reward(DEFAULT_WRAPPED_CURRENCY, fixed!(2344)));
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &BOB), 1289);
		assert_ok!(Reward::withdraw_reward(&ALICE, DEFAULT_WRAPPED_CURRENCY), 1054);
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &BOB), 1289);
	})
}

#[test]
fn should_withdraw_stake() {
	run_test(|| {
		assert_ok!(Reward::deposit_stake(&ALICE, fixed!(1312), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::distribute_reward(DEFAULT_WRAPPED_CURRENCY, fixed!(4242)));
		// rounding in `CheckedDiv` loses some precision
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &ALICE), 4241);
		assert_ok!(Reward::withdraw_stake(&ALICE, fixed!(1312), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &ALICE), 4241);
	})
}

#[test]
fn should_not_withdraw_stake_if_balance_insufficient() {
	run_test(|| {
		assert_ok!(Reward::deposit_stake(&ALICE, fixed!(100), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::distribute_reward(DEFAULT_WRAPPED_CURRENCY, fixed!(2000)));
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &ALICE), 2000);
		assert_err!(
			Reward::withdraw_stake(&ALICE, fixed!(200), DEFAULT_WRAPPED_CURRENCY),
			TestError::InsufficientFunds
		);
	})
}

#[test]
fn should_deposit_stake() {
	run_test(|| {
		assert_ok!(Reward::deposit_stake(&ALICE, fixed!(25), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::deposit_stake(&ALICE, fixed!(25), DEFAULT_WRAPPED_CURRENCY));
		assert_eq!(Reward::stake(&ALICE), fixed!(50));
		assert_ok!(Reward::deposit_stake(&BOB, fixed!(50), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::distribute_reward(DEFAULT_WRAPPED_CURRENCY, fixed!(1000)));
		assert_ok!(Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &ALICE), 500);
	})
}

#[test]
fn should_not_distribute_rewards_without_stake() {
	run_test(|| {
		assert_err!(
			Reward::distribute_reward(DEFAULT_WRAPPED_CURRENCY, fixed!(1000)),
			TestError::ZeroTotalStake
		);
		assert_eq!(Reward::total_rewards(DEFAULT_WRAPPED_CURRENCY), fixed!(0));
	})
}

#[test]
fn should_distribute_with_many_rewards() {
	// test that reward tally doesn't overflow
	run_test(|| {
		let mut rng = rand::thread_rng();
		assert_ok!(Reward::deposit_stake(&ALICE, fixed!(9230404), DEFAULT_WRAPPED_CURRENCY));
		assert_ok!(Reward::deposit_stake(&BOB, fixed!(234234444), DEFAULT_WRAPPED_CURRENCY));
		for _ in 0..30 {
			// NOTE: this will overflow compute_reward with > u32
			assert_ok!(Reward::distribute_reward(
				DEFAULT_WRAPPED_CURRENCY,
				fixed!(rng.gen::<u32>() as i128)
			));
		}
		let alice_reward = Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &ALICE).unwrap();
		assert_ok!(Reward::withdraw_reward(&ALICE, DEFAULT_WRAPPED_CURRENCY), alice_reward);
		let bob_reward = Reward::compute_reward(DEFAULT_WRAPPED_CURRENCY, &BOB).unwrap();
		assert_ok!(Reward::withdraw_reward(&BOB, DEFAULT_WRAPPED_CURRENCY), bob_reward);
	})
}
