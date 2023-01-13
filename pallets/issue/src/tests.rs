use frame_support::{assert_noop, assert_ok, dispatch::DispatchError};
use orml_traits::MultiCurrency;
use sp_arithmetic::FixedU128;
use sp_core::H256;
use sp_runtime::traits::{One, Zero};

use currency::Amount;
use mocktopus::mocking::*;
use primitives::{issue::IssueRequestStatus, StellarPublicKeyRaw};
use stellar_relay::testing_utils::{DEFAULT_STELLAR_PUBLIC_KEY, RANDOM_STELLAR_PUBLIC_KEY};
use vault_registry::{DefaultVault, DefaultVaultId, Vault, VaultStatus};

use crate::{ext, mock::*, BalanceOf, Event, IssueRequest};

fn griefing(amount: u128) -> Amount<Test> {
	Amount::new(amount, DEFAULT_NATIVE_CURRENCY)
}

fn wrapped(amount: u128) -> Amount<Test> {
	Amount::new(amount, DEFAULT_WRAPPED_CURRENCY)
}

fn request_issue(
	origin: AccountId,
	amount: Balance,
	vault: DefaultVaultId<Test>,
) -> Result<H256, DispatchError> {
	ext::security::get_secure_id::<Test>.mock_safe(|_| MockResult::Return(get_dummy_request_id()));

	ext::vault_registry::try_increase_to_be_issued_tokens::<Test>
		.mock_safe(|_, _| MockResult::Return(Ok(())));

	Issue::_request_issue(origin, amount, vault)
}

fn request_issue_ok(origin: AccountId, amount: Balance, vault: DefaultVaultId<Test>) -> H256 {
	request_issue_ok_with_address(origin, amount, vault, RANDOM_STELLAR_PUBLIC_KEY).unwrap()
}

fn request_issue_ok_with_address(
	origin: AccountId,
	amount: Balance,
	vault: DefaultVaultId<Test>,
	_address: StellarPublicKeyRaw,
) -> Result<H256, DispatchError> {
	ext::vault_registry::ensure_not_banned::<Test>.mock_safe(|_| MockResult::Return(Ok(())));

	ext::security::get_secure_id::<Test>.mock_safe(|_| MockResult::Return(get_dummy_request_id()));

	ext::vault_registry::try_increase_to_be_issued_tokens::<Test>
		.mock_safe(|_, _| MockResult::Return(Ok(())));
	ext::vault_registry::get_stellar_public_key::<Test>
		.mock_safe(|_| MockResult::Return(Ok(DEFAULT_STELLAR_PUBLIC_KEY)));

	Issue::_request_issue(origin, amount, vault)
}

fn execute_issue(origin: AccountId, issue_id: &H256) -> Result<(), DispatchError> {
	let (tx_env_encoded, scp_envelopes_encoded, transaction_set_encoded) =
		stellar_relay::testing_utils::create_dummy_scp_structs_encoded();

	Issue::_execute_issue(
		origin,
		*issue_id,
		tx_env_encoded,
		scp_envelopes_encoded,
		transaction_set_encoded,
	)
}

fn cancel_issue(origin: AccountId, issue_id: &H256) -> Result<(), DispatchError> {
	Issue::_cancel_issue(origin, *issue_id)
}

fn init_zero_vault(id: DefaultVaultId<Test>) -> DefaultVault<Test> {
	Vault::new(id)
}

fn get_dummy_request_id() -> H256 {
	H256::zero()
}

#[test]
fn test_request_issue_banned_fails() {
	run_test(|| {
		assert_ok!(<oracle::Pallet<Test>>::_set_exchange_rate(1,
			DEFAULT_COLLATERAL_CURRENCY,
			FixedU128::one()
		));
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 0,
				to_be_redeemed_tokens: 0,
				replace_collateral: 0,
				active_replace_collateral: 0,
				banned_until: Some(1),
				secure_collateral_threshold: None,
				status: VaultStatus::Active(true),
				liquidated_collateral: 0,
			},
		);
		assert_noop!(request_issue(USER, 3, VAULT), VaultRegistryError::VaultBanned);
	})
}

#[test]
fn test_request_issue_succeeds() {
	run_test(|| {
		let origin = USER;
		let vault = VAULT;
		let amount: Balance = 3;
		let issue_asset = vault.wrapped_currency();
		let issue_fee = 1;
		let issue_griefing_collateral = 20;
		let address = DEFAULT_STELLAR_PUBLIC_KEY;

		ext::vault_registry::get_active_vault_from_id::<Test>
			.mock_safe(|_| MockResult::Return(Ok(init_zero_vault(VAULT))));

		ext::fee::get_issue_fee::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(wrapped(issue_fee))));

		ext::fee::get_issue_griefing_collateral::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(griefing(issue_griefing_collateral))));

		let issue_id =
			request_issue_ok_with_address(origin, amount, vault.clone(), address).unwrap();

		let request_issue_event = TestEvent::Issue(Event::RequestIssue {
			issue_id,
			requester: origin,
			amount: amount - issue_fee,
			asset: issue_asset,
			fee: issue_fee,
			griefing_collateral: issue_griefing_collateral,
			vault_id: vault,
			vault_stellar_public_key: address,
		});
		assert!(System::events().iter().any(|a| a.event == request_issue_event));
	})
}

#[test]
fn test_execute_issue_not_found_fails() {
	run_test(|| {
		ext::vault_registry::get_active_vault_from_id::<Test>
			.mock_safe(|_| MockResult::Return(Ok(init_zero_vault(VAULT))));
		assert_noop!(execute_issue(USER, &H256([0; 32])), TestError::IssueIdNotFound);
	})
}

fn setup_execute(
	issue_amount: Balance,
	issue_fee: Balance,
	griefing_collateral: Balance,
	amount_transferred: Balance,
) -> Result<H256, DispatchError> {
	ext::vault_registry::get_active_vault_from_id::<Test>
		.mock_safe(|_| MockResult::Return(Ok(init_zero_vault(VAULT))));
	ext::vault_registry::issue_tokens::<Test>.mock_safe(|_, _| MockResult::Return(Ok(())));
	ext::vault_registry::is_vault_liquidated::<Test>.mock_safe(|_| MockResult::Return(Ok(false)));

	ext::fee::get_issue_fee::<Test>.mock_safe(move |_| MockResult::Return(Ok(wrapped(issue_fee))));
	ext::fee::get_issue_griefing_collateral::<Test>
		.mock_safe(move |_| MockResult::Return(Ok(griefing(griefing_collateral))));

	let issue_id =
		request_issue_ok_with_address(USER, issue_amount, VAULT, RANDOM_STELLAR_PUBLIC_KEY)?;
	<security::Pallet<Test>>::set_active_block_number(5);

	ext::stellar_relay::validate_stellar_transaction::<Test>
		.mock_safe(move |_, _, _| MockResult::Return(Ok(())));
	ext::currency::get_amount_from_transaction_envelope::<Test>.mock_safe(move |_, _, currency| {
		MockResult::Return(Ok(Amount::new(amount_transferred, currency)))
	});

	Ok(issue_id)
}

#[test]
fn test_execute_issue_succeeds() {
	run_test(|| {
		let issue_asset = VAULT.wrapped_currency();
		let issue_amount = 3;
		let issue_fee = 1;
		let griefing_collateral = 1;
		let amount_transferred = 3;
		let issue_id =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred)
				.unwrap();

		assert_ok!(execute_issue(USER, &issue_id));

		let execute_issue_event = TestEvent::Issue(Event::ExecuteIssue {
			issue_id,
			requester: USER,
			vault_id: VAULT,
			amount: issue_amount,
			asset: issue_asset,
			fee: issue_fee,
		});
		assert!(System::events().iter().any(|a| a.event == execute_issue_event));
		let executed_issue: IssueRequest<AccountId, BlockNumber, Balance, CurrencyId> =
			Issue::issue_requests(&issue_id).unwrap();
		assert!(matches!(executed_issue, IssueRequest { .. }));
		assert_eq!(executed_issue.amount, issue_amount - issue_fee);
		assert_eq!(executed_issue.fee, issue_fee);
		assert_eq!(executed_issue.griefing_collateral, griefing_collateral);

		assert_noop!(cancel_issue(USER, &issue_id), TestError::IssueCompleted);
	})
}

#[test]
fn test_execute_issue_overpayment_succeeds() {
	run_test(|| {
		let _issue_asset = VAULT.wrapped_currency();
		let issue_amount = 3;
		let amount_transferred = 5;
		let issue_fee = 0;
		let griefing_collateral = 0;
		let issue_id =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred)
				.unwrap();
		unsafe {
			let mut increase_tokens_called = false;

			ext::vault_registry::get_issuable_tokens_from_vault::<Test>
				.mock_raw(|_| MockResult::Return(Ok(wrapped(10))));
			ext::vault_registry::try_increase_to_be_issued_tokens::<Test>.mock_raw(|_, amount| {
				increase_tokens_called = true;
				assert_eq!(amount, &wrapped(2));
				MockResult::Return(Ok(()))
			});

			assert_ok!(execute_issue(USER, &issue_id));
			assert_eq!(increase_tokens_called, true);
			assert!(matches!(
				Issue::issue_requests(&issue_id),
				Some(IssueRequest {
					griefing_collateral: 0,
					amount: 5, // 3 + 2 - 0
					fee: 0,
					..
				})
			));
		}
	})
}

#[test]
fn test_execute_issue_overpayment_up_to_max_succeeds() {
	run_test(|| {
		let _issue_asset = VAULT.wrapped_currency();
		let issue_amount = 3;
		let amount_transferred = 10;
		let issue_fee = 0;
		let griefing_collateral = 0;
		let issue_id =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred)
				.unwrap();
		unsafe {
			let mut increase_tokens_called = false;

			ext::vault_registry::get_issuable_tokens_from_vault::<Test>
				.mock_raw(|_| MockResult::Return(Ok(wrapped(5))));
			ext::vault_registry::try_increase_to_be_issued_tokens::<Test>.mock_raw(|_, amount| {
				increase_tokens_called = true;
				assert_eq!(amount, &wrapped(5));
				MockResult::Return(Ok(()))
			});

			assert_ok!(execute_issue(USER, &issue_id));
			assert_eq!(increase_tokens_called, true);
			assert!(matches!(
				Issue::issue_requests(&issue_id),
				Some(IssueRequest {
					griefing_collateral: 0,
					amount: 8, // 3 + 5 - 0
					fee: 0,
					..
				})
			));
		}
	})
}

#[test]
fn test_execute_issue_underpayment_succeeds() {
	run_test(|| {
		let _issue_asset = VAULT.wrapped_currency();
		let issue_amount = 10;
		let amount_transferred = 1;
		let issue_fee = 0;
		let griefing_collateral = 20;
		let issue_id =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred)
				.unwrap();
		unsafe {
			let mut transfer_funds_called = false;
			ext::vault_registry::transfer_funds::<Test>.mock_raw(|from, to, amount| {
				transfer_funds_called = true;
				assert_eq!(from.account_id(), USER);
				assert_eq!(to.account_id(), VAULT.account_id);
				// to_release_collateral = (griefing_collateral * amount_transferred) /
				// expected_total_amount slashed_collateral = griefing_collateral -
				// to_release_collateral 20 - (20 * 1 / 10) = 18
				assert_eq!(amount, &griefing(18));
				MockResult::Return(Ok(()))
			});

			let mut decrease_to_be_issued_tokens_called = false;
			ext::vault_registry::decrease_to_be_issued_tokens::<Test>.mock_raw(|_, amount| {
				decrease_to_be_issued_tokens_called = true;
				assert_eq!(amount, &wrapped(9));
				MockResult::Return(Ok(()))
			});

			assert_ok!(execute_issue(USER, &issue_id));
			assert_eq!(decrease_to_be_issued_tokens_called, true);
			assert_eq!(transfer_funds_called, true);
			assert!(matches!(
				Issue::issue_requests(&issue_id),
				Some(IssueRequest {
					// NOTE: griefing collateral is not updated
					griefing_collateral: 20,
					amount: 1,
					fee: 0,
					..
				})
			));
		}
	})
}

#[test]
fn test_cancel_issue_not_found_fails() {
	run_test(|| {
		assert_noop!(cancel_issue(USER, &H256([0; 32])), TestError::IssueIdNotFound,);
	})
}

#[test]
fn test_cancel_issue_not_expired_and_not_requester_fails() {
	run_test(|| {
		ext::vault_registry::get_active_vault_from_id::<Test>
			.mock_safe(|_| MockResult::Return(Ok(init_zero_vault(VAULT))));

		let issue_id = request_issue_ok(USER, 3, VAULT);
		// issue period is 10, we issued at block 1, so at block 5 the cancel should fail
		<security::Pallet<Test>>::set_active_block_number(5);
		assert_noop!(cancel_issue(3, &issue_id), TestError::TimeNotExpired);
	})
}

#[test]
fn test_cancel_issue_not_expired_and_requester_succeeds() {
	run_test(|| {
		ext::vault_registry::get_active_vault_from_id::<Test>
			.mock_safe(|_| MockResult::Return(Ok(init_zero_vault(VAULT))));
		ext::vault_registry::decrease_to_be_issued_tokens::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(())));
		ext::vault_registry::is_vault_liquidated::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(false)));
		ext::fee::get_issue_griefing_collateral::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(griefing(100))));

		let issue_id = request_issue_ok(USER, 300, VAULT);

		unsafe {
			let mut transfer_called = false;

			// issue period is 10, we issued at block 1, so at block 4 the requester gets 70%
			// griefing back
			<security::Pallet<Test>>::set_active_block_number(4);

			let free_before = Tokens::free_balance(DEFAULT_NATIVE_CURRENCY, &USER);
			ext::vault_registry::transfer_funds::<Test>.mock_raw(|_, _, amount| {
				transfer_called = true;
				assert_eq!(amount, &griefing(30));
				MockResult::Return(Ok(()))
			});

			assert_ok!(cancel_issue(USER, &issue_id));

			assert_eq!(transfer_called, true);
			assert_eq!(Tokens::free_balance(DEFAULT_NATIVE_CURRENCY, &USER), free_before + 70);
			assert_eq!(
				Issue::issue_requests(&issue_id).unwrap().status,
				IssueRequestStatus::Cancelled
			);
		}
	})
}

#[test]
fn test_cancel_issue_expired_succeeds() {
	run_test(|| {
		ext::vault_registry::get_active_vault_from_id::<Test>
			.mock_safe(|_| MockResult::Return(Ok(init_zero_vault(VAULT))));
		ext::vault_registry::decrease_to_be_issued_tokens::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(())));
		ext::vault_registry::is_vault_liquidated::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(false)));
		ext::fee::get_issue_griefing_collateral::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(griefing(100))));

		let issue_id = request_issue_ok(USER, 300, VAULT);

		unsafe {
			// issue period is 10, we issued at block 1, so at block 12 the request has expired
			<security::Pallet<Test>>::set_active_block_number(12);

			ext::vault_registry::transfer_funds::<Test>.mock_raw(|_, _, amount| {
				assert_eq!(amount, &griefing(100));
				MockResult::Return(Ok(()))
			});

			assert_ok!(cancel_issue(USER, &issue_id));

			assert_eq!(
				Issue::issue_requests(&issue_id).unwrap().status,
				IssueRequestStatus::Cancelled
			);
		}
	})
}

#[test]
fn test_set_issue_period_only_root() {
	run_test(|| {
		assert_noop!(
			Issue::set_issue_period(RuntimeOrigin::signed(USER), 1),
			DispatchError::BadOrigin
		);
		assert_ok!(Issue::set_issue_period(RuntimeOrigin::root(), 1));
	})
}

#[test]
fn test_request_issue_fails_exceed_limit_volume_for_issue_request() {
	run_test(|| {
		let volume_limit = 1u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			DEFAULT_COLLATERAL_CURRENCY,
			7200u64,
		);

		let issue_amount = volume_limit + 1;
		let issue_fee = 1;
		let griefing_collateral = 1;
		let amount_transferred = issue_amount;
		let request_issue_result =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred);

		assert_noop!(request_issue_result, TestError::ExceedLimitVolumeForIssueRequest);
	})
}

#[test]
fn test_request_issue_fails_after_execute_issue_exceed_limit_volume_for_issue_request() {
	run_test(|| {
		let volume_limit = 3u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			DEFAULT_COLLATERAL_CURRENCY,
			7200u64,
		);

		let issue_asset = VAULT.wrapped_currency();
		let issue_amount = volume_limit;
		let issue_fee = 1;
		let griefing_collateral = 1;
		let amount_transferred = issue_amount;
		let request_issue_result =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred);

		assert_ok!(request_issue_result);
		let issue_id = request_issue_result.unwrap();

		assert_ok!(execute_issue(USER, &issue_id));

		let execute_issue_event = TestEvent::Issue(Event::ExecuteIssue {
			issue_id,
			requester: USER,
			vault_id: VAULT,
			amount: issue_amount,
			asset: issue_asset,
			fee: issue_fee,
		});
		assert!(System::events().iter().any(|a| a.event == execute_issue_event));
		let executed_issue: IssueRequest<AccountId, BlockNumber, Balance, CurrencyId> =
			Issue::issue_requests(&issue_id).unwrap();
		assert!(matches!(executed_issue, IssueRequest { .. }));
		assert_eq!(executed_issue.amount, issue_amount - issue_fee);
		assert_eq!(executed_issue.fee, issue_fee);
		assert_eq!(executed_issue.griefing_collateral, griefing_collateral);

		assert_noop!(cancel_issue(USER, &issue_id), TestError::IssueCompleted);

		//act
		// Try to request issue again although the volume limit has been reached
		let request_issue_result =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred);

		//assert check that we do not exceed the rate limit after execute issue request
		assert_noop!(request_issue_result, TestError::ExceedLimitVolumeForIssueRequest);
	})
}

#[test]
fn test_request_issue_success_with_rate_limit() {
	run_test(|| {
		let volume_limit = 3u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			DEFAULT_COLLATERAL_CURRENCY,
			7200u64,
		);

		let issue_asset = VAULT.wrapped_currency();
		let issue_amount = volume_limit;
		let issue_fee = 1;
		let griefing_collateral = 1;
		let amount_transferred = issue_amount;
		let request_issue_result =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred);

		assert_ok!(request_issue_result);
		let issue_id = request_issue_result.unwrap();

		assert_ok!(execute_issue(USER, &issue_id));

		let execute_issue_event = TestEvent::Issue(Event::ExecuteIssue {
			issue_id,
			requester: USER,
			vault_id: VAULT,
			amount: issue_amount,
			asset: issue_asset,
			fee: issue_fee,
		});
		assert!(System::events().iter().any(|a| a.event == execute_issue_event));
		let executed_issue: IssueRequest<AccountId, BlockNumber, Balance, CurrencyId> =
			Issue::issue_requests(&issue_id).unwrap();
		assert!(matches!(executed_issue, IssueRequest { .. }));
		assert_eq!(executed_issue.amount, issue_amount - issue_fee);
		assert_eq!(executed_issue.fee, issue_fee);
		assert_eq!(executed_issue.griefing_collateral, griefing_collateral);

		assert_noop!(cancel_issue(USER, &issue_id), TestError::IssueCompleted);
	})
}

#[test]
fn test_request_issue_reset_interval_and_succeeds_with_rate_limit() {
	run_test(|| {
		let volume_limit = 3u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			DEFAULT_COLLATERAL_CURRENCY,
			7200u64,
		);

		let issue_asset = VAULT.wrapped_currency();
		let issue_amount = volume_limit;
		let issue_fee = 1;
		let griefing_collateral = 1;
		let amount_transferred = issue_amount;
		let request_issue_result =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred);

		assert_ok!(request_issue_result);
		let issue_id = request_issue_result.unwrap();

		assert_ok!(execute_issue(USER, &issue_id));

		let execute_issue_event = TestEvent::Issue(Event::ExecuteIssue {
			issue_id,
			requester: USER,
			vault_id: VAULT,
			amount: issue_amount,
			asset: issue_asset,
			fee: issue_fee,
		});
		assert!(System::events().iter().any(|a| a.event == execute_issue_event));
		let executed_issue: IssueRequest<AccountId, BlockNumber, Balance, CurrencyId> =
			Issue::issue_requests(&issue_id).unwrap();
		assert!(matches!(executed_issue, IssueRequest { .. }));
		assert_eq!(executed_issue.amount, issue_amount - issue_fee);
		assert_eq!(executed_issue.fee, issue_fee);
		assert_eq!(executed_issue.griefing_collateral, griefing_collateral);

		assert_noop!(cancel_issue(USER, &issue_id), TestError::IssueCompleted);

		//act
		let request_issue_result =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred);

		//assert check that we do not exceed the rate limit after execute issue request
		assert_noop!(request_issue_result, TestError::ExceedLimitVolumeForIssueRequest);

		System::set_block_number(7200 + 20);

		// The volume limit does not automatically reset when the block number changes.
		// We need to request a new issue so that the rate limit is reset for the new interval.
		assert!(<crate::CurrentVolumeAmount<Test>>::get() > BalanceOf::<Test>::zero());

		//act
		let request_issue_result =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred);

		assert_ok!(request_issue_result);

		// The volume limit should be reset after the new issue request.
		// It is 0 because the issue was only requested and not yet executed.
		assert_eq!(<crate::CurrentVolumeAmount<Test>>::get(), BalanceOf::<Test>::zero());
	})
}
