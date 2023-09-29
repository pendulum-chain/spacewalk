use crate::{mock::*, CurrencyId, OracleKey};
use frame_support::{assert_err, assert_ok};
use mocktopus::mocking::*;
use sp_arithmetic::FixedU128;
use sp_runtime::FixedPointNumber;

fn mine_block() {
	crate::Pallet::<Test>::begin_block(0);
}

#[test]
fn feed_values_succeeds() {
	run_test(|| {
		let key = OracleKey::ExchangeRate(CurrencyId::AlphaNum4(
			*b"MXN\0",
			[
				20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231,
				46, 199, 108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
			],
		));
		let rate = FixedU128::checked_from_rational(100, 1).unwrap();

		let result = Oracle::_feed_values(3, vec![(key.clone(), rate)]);
		assert_ok!(result);

		mine_block();

		let exchange_rate = Oracle::get_price(key.clone()).unwrap();
		assert_eq!(exchange_rate, rate);
	});
}

mod oracle_offline_detection {
	use super::*;

	type SecurityPallet = security::Pallet<Test>;
	use security::StatusCode;

	enum SubmittingOracle {
		OracleA,
		OracleB,
	}
	use SubmittingOracle::*;

	fn set_time(time: u64) {
		Oracle::get_current_time.mock_safe(move || MockResult::Return(time));
		mine_block();
	}

	fn feed_value(currency_id: CurrencyId, _oracle: SubmittingOracle) {
		assert_ok!(Oracle::_feed_values(
			1,
			vec![(OracleKey::ExchangeRate(currency_id), FixedU128::from(1))]
		));
		mine_block();
	}
	fn feed_value_with_value(currency_id: CurrencyId, _oracle: SubmittingOracle, value: u128) {
		assert_ok!(Oracle::_feed_values(
			1,
			vec![(OracleKey::ExchangeRate(currency_id), FixedU128::from(value))]
		));
		mine_block();
	}

	#[test]
	fn basic_oracle_offline_logic() {
		run_test(|| {
			Oracle::get_max_delay.mock_safe(move || MockResult::Return(10));

			set_time(0);
			feed_value(CurrencyId::XCM(0), OracleA);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Running);

			set_time(5);
			feed_value(CurrencyId::XCM(1), OracleA);

			// DOT expires after block 10
			set_time(10);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Running);
			set_time(11);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Error);

			// feeding KSM makes no difference
			feed_value(CurrencyId::XCM(1), OracleA);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Error);

			// feeding DOT makes it running again
			feed_value_with_value(CurrencyId::XCM(0), OracleA, 7);
			set_time(15);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Running);

			// KSM expires after t=21 (it was set at t=11)
			set_time(21);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Running);
			set_time(22);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Error);

			// check that status remains ERROR until BOTH currencies have been updated
			set_time(100);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Error);
			feed_value(CurrencyId::XCM(0), OracleA);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Error);
			feed_value(CurrencyId::XCM(1), OracleA);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Running);
		});
	}

	#[test]
	fn oracle_offline_logic_with_multiple_oracles() {
		run_test(|| {
			Oracle::get_max_delay.mock_safe(move || MockResult::Return(10));

			set_time(0);
			feed_value(CurrencyId::XCM(0), OracleA);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Running);

			set_time(5);
			feed_value(CurrencyId::XCM(1), OracleA);

			set_time(7);
			feed_value(CurrencyId::XCM(0), OracleB);

			// OracleA's DOT submission expires at 10, but OracleB's only at 17. However, KSM
			// expires at 15:
			set_time(15);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Running);
			set_time(16);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Error);

			// Feeding KSM brings it back online
			feed_value(CurrencyId::XCM(1), OracleA);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Running);

			// check that status is set of ERROR when both oracle's DOT submission expired
			set_time(17);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Running);
			set_time(18);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Error);

			// A DOT submission by any oracle brings it back online
			feed_value(CurrencyId::XCM(0), OracleA);
			assert_eq!(SecurityPallet::parachain_status(), StatusCode::Running);
		});
	}
}

#[test]
fn getting_exchange_rate_fails_with_missing_exchange_rate() {
	run_test(|| {
		let key = OracleKey::ExchangeRate(CurrencyId::XCM(0));
		assert_err!(Oracle::get_price(key), TestError::MissingExchangeRate);
		assert_err!(Oracle::currency_to_usd(0, CurrencyId::XCM(0)), TestError::MissingExchangeRate);
		assert_err!(Oracle::usd_to_currency(0, CurrencyId::XCM(0)), TestError::MissingExchangeRate);
		assert_err!(Oracle::get_exchange_rate(CurrencyId::XCM(0)), TestError::MissingExchangeRate);
	});
}

#[test]
fn currency_to_usd() {
	run_test(|| {
		Oracle::get_price
			.mock_safe(|_| MockResult::Return(Ok(FixedU128::checked_from_rational(2, 1).unwrap())));
		let test_cases = [(0, 0), (2, 4), (10, 20)];
		for (input, expected) in test_cases.iter() {
			let result = Oracle::currency_to_usd(*input, CurrencyId::XCM(0));
			assert_ok!(result, *expected);
		}
	});
}

#[test]
fn usd_to_currency() {
	run_test(|| {
		Oracle::get_price
			.mock_safe(|_| MockResult::Return(Ok(FixedU128::checked_from_rational(2, 1).unwrap())));
		let test_cases = [(0, 0), (4, 2), (20, 10), (21, 10)];
		for (input, expected) in test_cases.iter() {
			let result = Oracle::usd_to_currency(*input, CurrencyId::XCM(0));
			assert_ok!(result, *expected);
		}
	});
}

#[test]
fn get_exchange_rate() {
	run_test(|| {
		Oracle::get_price
			.mock_safe(|_| MockResult::Return(Ok(FixedU128::checked_from_rational(9, 4).unwrap())));

		let result = Oracle::get_exchange_rate(CurrencyId::XCM(0));
		println!("{:?}", result);
		assert_ok!(result, FixedPointNumber::checked_from_rational(9, 4).unwrap());
	});
}

#[test]
fn test_is_invalidated() {
	run_test(|| {
		let now = 1585776145;
		Oracle::get_current_time.mock_safe(move || MockResult::Return(now));
		Oracle::get_max_delay.mock_safe(|| MockResult::Return(3600));

		let key = OracleKey::ExchangeRate(CurrencyId::XCM(0));
		let rate = FixedU128::checked_from_rational(100, 1).unwrap();

		assert_ok!(Oracle::_feed_values(3, vec![(key.clone(), rate)]));
		mine_block();

		// max delay is 60 minutes, 60+ passed
		// assert!(Oracle::is_outdated(&key, now + 3601));//TODO

		// max delay is 60 minutes, 30 passed
		Oracle::get_current_time.mock_safe(move || MockResult::Return(now + 1800));
		// assert!(!Oracle::is_outdated(&key, now + 3599)); //TODO
	});
}

#[test]
fn begin_block_set_oracle_offline_succeeds() {
	run_test(|| unsafe {
		let mut oracle_reported = false;
		Oracle::report_oracle_offline.mock_raw(|| {
			oracle_reported = true;
			MockResult::Return(())
		});

		Oracle::begin_block(0);
		assert!(oracle_reported, "Oracle should be reported as offline");
	});
}
