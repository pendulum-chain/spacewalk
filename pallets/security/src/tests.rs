use crate::{mock::*, ErrorCode, StatusCode};
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;

type Event = crate::Event<Test>;

macro_rules! assert_emitted {
	($event:expr) => {
		let test_event = TestEvent::Security($event);
		assert!(System::events().iter().any(|a| a.event == test_event));
	};
	($event:expr, $times:expr) => {
		let test_event = TestEvent::Security($event);
		assert_eq!(System::events().iter().filter(|a| a.event == test_event).count(), $times);
	};
}

#[test]
fn test_get_and_set_status() {
	run_test(|| {
		let status_code = Security::parachain_status();
		assert_eq!(status_code, StatusCode::Running);
		Security::set_status(StatusCode::Shutdown);
		let status_code = Security::parachain_status();
		assert_eq!(status_code, StatusCode::Shutdown);
	})
}

#[test]
fn test_is_ensure_parachain_running_succeeds() {
	run_test(|| {
		Security::set_status(StatusCode::Running);
		assert_ok!(Security::ensure_parachain_status_running());
	})
}

#[test]
fn test_is_ensure_parachain_running_fails() {
	run_test(|| {
		Security::set_status(StatusCode::Error);
		assert_noop!(Security::ensure_parachain_status_running(), TestError::ParachainNotRunning);

		Security::set_status(StatusCode::Shutdown);
		assert_noop!(Security::ensure_parachain_status_running(), TestError::ParachainNotRunning);
	})
}

#[test]
fn test_is_parachain_shutdown_succeeds() {
	run_test(|| {
		Security::set_status(StatusCode::Running);
		assert!(!Security::is_parachain_shutdown());

		Security::set_status(StatusCode::Error);
		assert!(!Security::is_parachain_shutdown());

		Security::set_status(StatusCode::Shutdown);
		assert!(Security::is_parachain_shutdown());
	})
}

#[test]
fn test_is_parachain_error_oracle_offline() {
	run_test(|| {
		Security::set_status(StatusCode::Error);
		Security::insert_error(ErrorCode::OracleOffline);
		assert!(Security::is_parachain_error_oracle_offline());
	})
}

fn test_recover_from_<F>(recover: F, error_codes: Vec<ErrorCode>)
where
	F: FnOnce(),
{
	for err in &error_codes {
		Security::insert_error(err.clone());
	}
	recover();
	for err in &error_codes {
		assert!(!Security::get_errors().contains(err));
	}
	assert_eq!(Security::parachain_status(), StatusCode::Running);
	assert_emitted!(Event::RecoverFromErrors {
		new_status: StatusCode::Running,
		cleared_errors: error_codes
	});
}

#[test]
fn test_recover_from_oracle_offline_succeeds() {
	run_test(|| {
		test_recover_from_(Security::recover_from_oracle_offline, vec![ErrorCode::OracleOffline]);
	})
}

#[test]
fn test_get_secure_id() {
	run_test(|| {
		frame_system::Pallet::<Test>::set_parent_hash(H256::zero());
		frame_system::Pallet::<Test>::set_extrinsic_index(1);
		assert_eq!(
			Security::get_secure_id(),
			H256::from_slice(&[
				40, 112, 84, 231, 187, 52, 6, 86, 237, 236, 56, 165, 29, 144, 84, 200, 137, 29, 55,
				76, 146, 12, 6, 38, 134, 82, 234, 213, 80, 192, 222, 92,
			])
		);
	})
}

#[test]
fn test_get_increment_active_block_succeeds() {
	run_test(|| {
		let initial_active_block = Security::active_block_number();
		Security::set_status(StatusCode::Running);
		Security::increment_active_block();
		assert_eq!(Security::active_block_number(), initial_active_block + 1);
	})
}

#[test]
fn test_get_active_block_not_incremented_if_not_running() {
	run_test(|| {
		let initial_active_block = Security::active_block_number();

		// not updated if there is an error
		Security::set_status(StatusCode::Error);
		Security::increment_active_block();
		assert_eq!(Security::active_block_number(), initial_active_block);

		// not updated if there is shutdown
		Security::set_status(StatusCode::Shutdown);
		Security::increment_active_block();
		assert_eq!(Security::active_block_number(), initial_active_block);
	})
}

mod spec_based_tests {
	use super::*;
	use sp_core::U256;

	#[test]
	fn test_generate_secure_id() {
		run_test(|| {
			let get_secure_id_with = |index, nonce: u32, parent| {
				crate::Nonce::<Test>::set(U256::from(nonce));

				frame_system::Pallet::<Test>::set_parent_hash(H256::from_slice(&[parent; 32]));
				frame_system::Pallet::<Test>::set_extrinsic_index(index);
				Security::get_secure_id()
			};

			let test_secure_id_with = |index, nonce: u32, parent| {
				let result1 = get_secure_id_with(index, nonce, parent);
				let result2 = get_secure_id_with(index, nonce, parent);
				// test that the result ONLY depend on account, nonce and parent
				assert_eq!(result1, result2);
				result1
			};

			let mut results = vec![];

			for i in 0..2 {
				for j in 0..2 {
					for k in 0..2 {
						let result = test_secure_id_with(i, j, k);
						results.push(result);
					}
				}
			}
			results.sort(); // required because dedup only remove duplicate _consecutive_ values
			results.dedup();

			// postcondition: MUST return the 256-bit hash of the account, nonce, and parent_hash
			// test that each combination of account, nonce, and parent_hash gives a unique result
			assert_eq!(results.len(), 8);

			// postcondition: Nonce MUST be incremented by one.
			let initial = crate::Nonce::<Test>::get();
			Security::get_secure_id();
			assert_eq!(crate::Nonce::<Test>::get(), initial + 1);
		})
	}

	#[test]
	fn test_has_expired() {
		run_test(|| {
			let test_parachain_block_expired_postcondition =
				|opentime, period, active_block_count| {
					// postcondition: MUST return True if opentime + period < ActiveBlockCount,
					// False otherwise.
					let expected = opentime + period < active_block_count;

					crate::ActiveBlockCount::<Test>::set(active_block_count);

					assert_eq!(
						expected,
						Security::parachain_block_expired(opentime, period).unwrap()
					)
				};

			for i in 0..4 {
				for j in 0..4 {
					for k in 1..3 {
						// precondition: The ActiveBlockCount MUST be greater than 0.
						test_parachain_block_expired_postcondition(i, j, k);
					}
				}
			}
		})
	}
}
