use frame_support::{assert_noop, assert_ok, sp_runtime::DispatchError};
use mocktopus::mocking::*;
use sp_arithmetic::FixedU128;
use sp_core::H256;
use sp_runtime::traits::{One, Zero};

use currency::Amount;
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
	ext::security::get_secure_id::<Test>.mock_safe(|| MockResult::Return(get_dummy_request_id()));

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

	ext::security::get_secure_id::<Test>.mock_safe(|| MockResult::Return(get_dummy_request_id()));

	ext::stellar_relay::ensure_transaction_memo_matches::<Test>
		.mock_safe(|_, _| MockResult::Return(Ok(())));

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
		assert_ok!(<oracle::Pallet<Test>>::_set_exchange_rate(
			1,
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

			let free_before = Balances::free_balance(&USER);
			ext::vault_registry::transfer_funds::<Test>.mock_raw(|_, _, amount| {
				transfer_called = true;
				assert_eq!(amount, &griefing(30));
				MockResult::Return(Ok(()))
			});

			assert_ok!(cancel_issue(USER, &issue_id));

			assert_eq!(transfer_called, true);
			assert_eq!(Balances::free_balance(&USER), free_before + 70);
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
		let volume_currency = DEFAULT_COLLATERAL_CURRENCY;
		let volume_limit = 1_000u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			volume_currency,
			7200u64,
		);

		let issue_asset = VAULT.wrapped_currency();
		// First, convert the volume limit to the issue asset
		let volume_limit_denoted_in_wrapped_asset =
			Oracle::convert(&Amount::new(volume_limit, volume_currency), issue_asset)
				.expect("Price conversion should work");

		// We set the issue amount as the volume limit + 100 (we have to use at least 100 because
		// the issue_amount might get rounded down during the price conversion between assets with
		// decimals differing by 2)
		let issue_amount = volume_limit_denoted_in_wrapped_asset.amount() + 100;
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
		let volume_currency = DEFAULT_COLLATERAL_CURRENCY;
		let volume_limit = 3u128;
		crate::Pallet::<Test>::_rate_limit_update(
			Option::<u128>::Some(volume_limit),
			volume_currency,
			7200u64,
		);

		let issue_asset = VAULT.wrapped_currency();

		// We set the issue amount as exactly the volume limit
		// First, convert the volume limit to the issue asset
		let volume_limit_denoted_in_wrapped_asset =
			Oracle::convert(&Amount::new(volume_limit, volume_currency), issue_asset)
				.expect("Price conversion should work");
		let issue_amount = volume_limit_denoted_in_wrapped_asset.amount();
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
		let volume_currency = DEFAULT_COLLATERAL_CURRENCY;
		let volume_limit = 3u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			volume_currency,
			7200u64,
		);

		let issue_asset = VAULT.wrapped_currency();

		// We set the issue amount as exactly the volume limit
		let volume_limit_denoted_in_wrapped_asset =
			Oracle::convert(&Amount::new(volume_limit, volume_currency), issue_asset)
				.expect("Price conversion should work");
		let issue_amount = volume_limit_denoted_in_wrapped_asset.amount();
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
		let volume_currency = DEFAULT_COLLATERAL_CURRENCY;
		let volume_limit = 3u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			volume_currency,
			7200u64,
		);

		let issue_asset = VAULT.wrapped_currency();
		let volume_limit_denoted_in_wrapped_asset =
			Oracle::convert(&Amount::new(volume_limit, volume_currency), issue_asset)
				.expect("Price conversion should work");

		let issue_amount = volume_limit_denoted_in_wrapped_asset.amount();
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
mod integration_tests {
	use super::*;
	use oracle::OracleApi;
	use orml_traits::MultiCurrency;
	use pooled_rewards::RewardsApi;

	fn get_reward_for_vault(vault: &DefaultVaultId<Test>, reward_currency: CurrencyId) -> Balance {
		<<Test as fee::Config>::VaultRewards as RewardsApi<
			CurrencyId,
			DefaultVaultId<Test>,
			Balance,
		>>::compute_reward(&vault.collateral_currency(), vault, reward_currency)
		.unwrap()
	}

	fn get_balance(currency_id: CurrencyId, account: &AccountId) -> Balance {
		<orml_currencies::Pallet<Test> as MultiCurrency<AccountId>>::free_balance(
			currency_id,
			account,
		)
	}
	fn nominate_vault(nominator: AccountId, vault: DefaultVaultId<Test>, amount: Balance) {
		let origin = RuntimeOrigin::signed(vault.account_id);

		assert_ok!(<nomination::Pallet<Test>>::opt_in_to_nomination(
			origin.clone(),
			vault.clone().currencies
		));

		let nominator_origin = RuntimeOrigin::signed(nominator);
		assert_ok!(<nomination::Pallet<Test>>::deposit_collateral(nominator_origin, vault, amount));
	}

	fn register_vault_with_collateral(vault: &DefaultVaultId<Test>, collateral_amount: Balance) {
		let origin = RuntimeOrigin::signed(vault.account_id);

		assert_ok!(VaultRegistry::register_public_key(origin.clone(), DEFAULT_STELLAR_PUBLIC_KEY));
		assert_ok!(VaultRegistry::register_vault(
			origin,
			vault.currencies.clone(),
			collateral_amount
		));
	}
	fn wrapped_with_custom_curr(amount: u128, currency: CurrencyId) -> Amount<Test> {
		Amount::new(amount, currency)
	}
	fn setup_execute_with_vault(
		issue_amount: Balance,
		issue_fee: Balance,
		griefing_collateral: Balance,
		amount_transferred: Balance,
		vault: DefaultVaultId<Test>,
	) -> Result<H256, DispatchError> {
		let vault_clone = vault.clone();
		let vault_clone_2 = vault.clone();
		ext::vault_registry::get_active_vault_from_id::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(init_zero_vault(vault_clone.clone()))));
		ext::vault_registry::issue_tokens::<Test>.mock_safe(|_, _| MockResult::Return(Ok(())));
		ext::vault_registry::is_vault_liquidated::<Test>
			.mock_safe(|_| MockResult::Return(Ok(false)));

		ext::fee::get_issue_fee::<Test>.mock_safe(move |_| {
			MockResult::Return(Ok(wrapped_with_custom_curr(
				issue_fee,
				vault_clone_2.wrapped_currency(),
			)))
		});
		ext::fee::get_issue_griefing_collateral::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(griefing(griefing_collateral))));

		let issue_id =
			request_issue_ok_with_address(USER, issue_amount, vault, RANDOM_STELLAR_PUBLIC_KEY)?;
		<security::Pallet<Test>>::set_active_block_number(5);

		ext::stellar_relay::validate_stellar_transaction::<Test>
			.mock_safe(move |_, _, _| MockResult::Return(Ok(())));
		ext::currency::get_amount_from_transaction_envelope::<Test>.mock_safe(
			move |_, _, currency| MockResult::Return(Ok(Amount::new(amount_transferred, currency))),
		);

		Ok(issue_id)
	}

	fn to_usd(amount: &Balance, currency: &CurrencyId) -> Balance {
		<Test as reward_distribution::Config>::OracleApi::currency_to_usd(amount, currency)
			.expect("prices have been set in mock configuration")
	}

	fn assert_approx_eq(a: Balance, b: Balance, epsilon: Balance) {
		assert!(
			(a as i128 - b as i128).abs() < epsilon as i128,
			"Values are not approximately equal: {} != {}",
			a,
			b
		);
	}

	//In these tests in particular we are testing that a given fee is distributed to
	//the pooled-reward pallet depending on the collateral.
	//These first 3 integration tests WILL NOT test the distribution from pooled-reward to staking.
	//The last one DOES
	#[test]
	fn integration_single_vault_gets_fee_rewards_when_issuing() {
		run_test(|| {
			//set up vault
			let collateral: u128 = 1000;
			register_vault_with_collateral(&VAULT, collateral);
			assert_ok!(Staking::add_reward_currency(VAULT.wrapped_currency()));
			//execute the issue

			let issue_asset = VAULT.wrapped_currency();
			let issue_amount = 30;
			let issue_fee = 1;
			let griefing_collateral = 1;
			let amount_transferred = 30;
			let issue_id =
				setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred)
					.unwrap();

			assert_ok!(execute_issue(USER, &issue_id));

			//ensure that the current rewards equal to fee
			assert_eq!(get_reward_for_vault(&VAULT, issue_asset), issue_fee);
		})
	}

	#[test]
	fn integration_multiple_vaults_same_collateral_gets_fee_rewards_when_issuing() {
		run_test(|| {
			//ARRANGE
			assert_ok!(Staking::add_reward_currency(VAULT.wrapped_currency()));
			assert_ok!(Staking::add_reward_currency(VAULT_2.wrapped_currency()));
			//set up the vaults
			let collateral_1: u128 = 1000;
			register_vault_with_collateral(&VAULT, collateral_1);

			let collateral_2: u128 = 2000;
			register_vault_with_collateral(&VAULT_2, collateral_2);

			//execute the issue
			let issue_asset_1 = VAULT.wrapped_currency();
			let issue_amount_1 = 30;
			let issue_fee_1 = 10;
			let griefing_collateral_1 = 1;
			let amount_transferred_1 = 30;
			let issue_id_1 = setup_execute(
				issue_amount_1,
				issue_fee_1,
				griefing_collateral_1,
				amount_transferred_1,
			)
			.unwrap();

			//ACT
			assert_ok!(execute_issue(USER, &issue_id_1));

			//execute the issue on the other currency
			let issue_asset_2 = VAULT_2.wrapped_currency();
			let issue_amount_2 = 30;
			let issue_fee_2 = 20;
			let griefing_collateral_2 = 1;
			let amount_transferred_2 = 30;
			let issue_id_2 = setup_execute_with_vault(
				issue_amount_2,
				issue_fee_2,
				griefing_collateral_2,
				amount_transferred_2,
				VAULT_2.clone(),
			)
			.unwrap();

			assert_ok!(execute_issue(USER, &issue_id_2));

			//ASSERT
			//ensure that the current rewards equal to fee
			//Example: we expect for reward(vault1, asset1) = floor( 10*(1000/3000) ) = 3
			let vault_1_collateral_usd = to_usd(&collateral_1, &VAULT.collateral_currency());
			let vault_2_collateral_usd = to_usd(&collateral_2, &VAULT_2.collateral_currency());
			let total_amount_usd = vault_1_collateral_usd + vault_2_collateral_usd;

			let expected_value_vault_1_fee_1: u128 =
				((vault_1_collateral_usd as f64 / total_amount_usd as f64) * issue_fee_1 as f64)
					.floor() as u128;

			let expected_value_vault_2_fee_1: u128 =
				((vault_2_collateral_usd as f64 / total_amount_usd as f64) * issue_fee_1 as f64)
					.floor() as u128;

			let expected_value_vault_1_fee_2: u128 =
				((vault_1_collateral_usd as f64 / total_amount_usd as f64) * issue_fee_2 as f64)
					.floor() as u128;

			let expected_value_vault_2_fee_2: u128 =
				((vault_2_collateral_usd as f64 / total_amount_usd as f64) * issue_fee_2 as f64)
					.floor() as u128;

			assert_eq!(get_reward_for_vault(&VAULT, issue_asset_1), expected_value_vault_1_fee_1);
			assert_eq!(get_reward_for_vault(&VAULT_2, issue_asset_1), expected_value_vault_2_fee_1);

			assert_eq!(get_reward_for_vault(&VAULT, issue_asset_2), expected_value_vault_1_fee_2);
			assert_eq!(get_reward_for_vault(&VAULT_2, issue_asset_2), expected_value_vault_2_fee_2);
		})
	}

	fn execute_multiple_issues() -> (Balance, Balance, Balance) {
		//execute the issue
		let issue_amount_1 = 3000;
		let issue_fee_1 = 1000;
		let griefing_collateral_1 = 1;
		let amount_transferred_1 = 3000;
		let issue_id_1 =
			setup_execute(issue_amount_1, issue_fee_1, griefing_collateral_1, amount_transferred_1)
				.unwrap();
		assert_ok!(execute_issue(USER, &issue_id_1));

		//execute the issue on the other currency
		let issue_amount_2 = 3000;
		let issue_fee_2 = 500;
		let griefing_collateral_2 = 1;
		let amount_transferred_2 = 3000;
		let issue_id_2 = setup_execute_with_vault(
			issue_amount_2,
			issue_fee_2,
			griefing_collateral_2,
			amount_transferred_2,
			VAULT_2.clone(),
		)
		.unwrap();
		assert_ok!(execute_issue(USER, &issue_id_2));

		let issue_amount_3 = 3000;
		let issue_fee_3 = 600;
		let griefing_collateral_3 = 1;
		let amount_transferred_3 = 3000;
		let issue_id_3 = setup_execute_with_vault(
			issue_amount_3,
			issue_fee_3,
			griefing_collateral_3,
			amount_transferred_3,
			VAULT_3.clone(),
		)
		.unwrap();
		assert_ok!(execute_issue(USER, &issue_id_3));

		let issue_amount_4 = 3000;
		let issue_fee_4 = 400;
		let griefing_collateral_4 = 1;
		let amount_transferred_4 = 3000;
		let issue_id_4 = setup_execute_with_vault(
			issue_amount_4,
			issue_fee_4,
			griefing_collateral_4,
			amount_transferred_4,
			VAULT_4.clone(),
		)
		.unwrap();
		assert_ok!(execute_issue(USER, &issue_id_4));

		let issue_amount_5 = 3000;
		let issue_fee_5 = 700;
		let griefing_collateral_5 = 1;
		let amount_transferred_5 = 3000;
		let issue_id_5 = setup_execute_with_vault(
			issue_amount_5,
			issue_fee_5,
			griefing_collateral_5,
			amount_transferred_5,
			VAULT_5.clone(),
		)
		.unwrap();
		assert_ok!(execute_issue(USER, &issue_id_5));

		//Vault 1 and 4 share and vault 2 and 5 share the same wrapped currency
		//so we must take into account the combined issuance of these pairs
		return (issue_fee_1 + issue_fee_4, issue_fee_2 + issue_fee_5, issue_fee_3);
	}

	#[test]
	fn integration_multiple_vaults_many_collateral() {
		run_test(|| {
			//ARRANGE
			assert_ok!(Staking::add_reward_currency(VAULT.wrapped_currency()));
			assert_ok!(Staking::add_reward_currency(VAULT_2.wrapped_currency()));
			assert_ok!(Staking::add_reward_currency(VAULT_3.wrapped_currency()));
			//set up the vaults
			let collateral_1: u128 = 1000;
			register_vault_with_collateral(&VAULT, collateral_1);

			let collateral_2: u128 = 2000;
			register_vault_with_collateral(&VAULT_2, collateral_2);

			let collateral_3: u128 = 1500;
			register_vault_with_collateral(&VAULT_3, collateral_3);

			let collateral_4: u128 = 1700;
			register_vault_with_collateral(&VAULT_4, collateral_4);

			let collateral_5: u128 = 800;
			register_vault_with_collateral(&VAULT_5, collateral_5);

			//execute issues
			let issue_asset_1 = VAULT.wrapped_currency();
			let issue_asset_2 = VAULT_2.wrapped_currency();
			let issue_asset_3 = VAULT_3.wrapped_currency();

			//ACT
			//get issue total for asset1, asset2 and asset3
			let (issue_fee_1, issue_fee_2, issue_fee_3) = execute_multiple_issues();

			//ASSERT
			//ensure that the current rewards equal to fee
			//Example: we expect for reward(vault1, asset1) = floor(
			//floor((1000/3000)*floor( 1400*(3000*5/(3000*5+3200*10+800*15)) )) = 118

			let vault_1_collateral_usd = to_usd(&collateral_1, &VAULT.collateral_currency());
			let vault_2_collateral_usd = to_usd(&collateral_2, &VAULT_2.collateral_currency());
			let vault_3_collateral_usd = to_usd(&collateral_3, &VAULT_3.collateral_currency());
			let vault_4_collateral_usd = to_usd(&collateral_4, &VAULT_4.collateral_currency());
			let vault_5_collateral_usd = to_usd(&collateral_5, &VAULT_5.collateral_currency());

			let total_amount_usd = vault_1_collateral_usd
				+ vault_2_collateral_usd
				+ vault_3_collateral_usd
				+ vault_4_collateral_usd
				+ vault_5_collateral_usd;

			// In order to calculate the value corresponding to each vault on the pool,
			// we must take into account that vault 1 and 2, vault 3 and 4 share the same
			// collateral, so in the calculations for the pooled rewards, these must be grouped
			let get_expected_value_vault_1 = |fee: Balance| -> Balance {
				((((vault_1_collateral_usd + vault_2_collateral_usd) as f64
					/ total_amount_usd as f64)
					* fee as f64)
					.floor() * (collateral_1 as f64 / (collateral_2 + collateral_1) as f64))
					.floor() as u128
			};

			let get_expected_value_vault_2 = |fee: Balance| -> Balance {
				((((vault_1_collateral_usd + vault_2_collateral_usd) as f64
					/ total_amount_usd as f64)
					* fee as f64)
					.floor() * (collateral_2 as f64 / (collateral_2 + collateral_1) as f64))
					.floor() as u128
			};

			let get_expected_value_vault_3 = |fee: Balance| -> Balance {
				((((vault_3_collateral_usd + vault_4_collateral_usd) as f64
					/ total_amount_usd as f64)
					* fee as f64)
					.floor() * (collateral_3 as f64 / (collateral_3 + collateral_4) as f64))
					.floor() as u128
			};

			let get_expected_value_vault_4 = |fee: Balance| -> Balance {
				((((vault_3_collateral_usd + vault_4_collateral_usd) as f64
					/ total_amount_usd as f64)
					* fee as f64)
					.floor() * (collateral_4 as f64 / (collateral_3 + collateral_4) as f64))
					.floor() as u128
			};

			let get_expected_value_vault_5 = |fee: Balance| -> Balance {
				(((vault_5_collateral_usd as f64 / total_amount_usd as f64) * fee as f64).floor())
					as u128
			};

			//assertions for asset1 = asset4
			assert_approx_eq(
				get_reward_for_vault(&VAULT, issue_asset_1),
				get_expected_value_vault_1(issue_fee_1),
				2,
			);

			assert_approx_eq(
				get_reward_for_vault(&VAULT_2, issue_asset_1),
				get_expected_value_vault_2(issue_fee_1),
				2,
			);
			assert_approx_eq(
				get_reward_for_vault(&VAULT_3, issue_asset_1),
				get_expected_value_vault_3(issue_fee_1),
				2,
			);
			//NOTE: withouth floor(), should actually exactly be 403.38. In this case
			//it is 402. The discrepancy is due to two consecutive roundings, and also
			//the precision by which the pallet pooled-rewards works.
			assert_approx_eq(
				get_reward_for_vault(&VAULT_4, issue_asset_1),
				get_expected_value_vault_4(issue_fee_1),
				2,
			);
			assert_approx_eq(
				get_reward_for_vault(&VAULT_5, issue_asset_1),
				get_expected_value_vault_5(issue_fee_1),
				2,
			);

			//assertions for issue asset2 = asset5
			assert_approx_eq(
				get_reward_for_vault(&VAULT, issue_asset_2),
				get_expected_value_vault_1(issue_fee_2),
				2,
			);
			assert_approx_eq(
				get_reward_for_vault(&VAULT_2, issue_asset_2),
				get_expected_value_vault_2(issue_fee_2),
				2,
			);
			assert_approx_eq(
				get_reward_for_vault(&VAULT_3, issue_asset_2),
				get_expected_value_vault_3(issue_fee_2),
				2,
			);
			assert_approx_eq(
				get_reward_for_vault(&VAULT_4, issue_asset_2),
				get_expected_value_vault_4(issue_fee_2),
				2,
			);
			assert_approx_eq(
				get_reward_for_vault(&VAULT_5, issue_asset_2),
				get_expected_value_vault_5(issue_fee_2),
				2,
			);

			//assertions for issue asset3
			assert_approx_eq(
				get_reward_for_vault(&VAULT, issue_asset_3),
				get_expected_value_vault_1(issue_fee_3),
				2,
			);
			assert_approx_eq(
				get_reward_for_vault(&VAULT_2, issue_asset_3),
				get_expected_value_vault_2(issue_fee_3),
				2,
			);
			assert_approx_eq(
				get_reward_for_vault(&VAULT_3, issue_asset_3),
				get_expected_value_vault_3(issue_fee_3),
				2,
			);
			assert_approx_eq(
				get_reward_for_vault(&VAULT_4, issue_asset_3),
				get_expected_value_vault_4(issue_fee_3),
				2,
			);
			assert_approx_eq(
				get_reward_for_vault(&VAULT_5, issue_asset_3),
				get_expected_value_vault_5(issue_fee_3),
				2,
			);
		});
	}

	//same issue and fee set up as previous test, but nominators are
	//taken into account.
	//only vault 1 is taken into account for testing nominator rewards
	#[test]
	fn integration_multiple_vaults_many_collateral_nominated() {
		run_test(|| {
			//ARRANGE
			assert_ok!(Staking::add_reward_currency(VAULT.wrapped_currency()));
			assert_ok!(Staking::add_reward_currency(VAULT_2.wrapped_currency()));
			assert_ok!(Staking::add_reward_currency(VAULT_3.wrapped_currency()));
			//set up the vaults
			let collateral_1: u128 = 1000;
			register_vault_with_collateral(&VAULT, collateral_1);

			let collateral_2: u128 = 2000;
			register_vault_with_collateral(&VAULT_2, collateral_2);

			let collateral_3: u128 = 1500;
			register_vault_with_collateral(&VAULT_3, collateral_3);

			let collateral_4: u128 = 1700;
			register_vault_with_collateral(&VAULT_4, collateral_4);

			let collateral_5: u128 = 800;
			register_vault_with_collateral(&VAULT_5, collateral_5);

			//nominate
			let nominator_amount: u128 = 500;

			nominate_vault(NOMINATOR1, VAULT, nominator_amount);

			//execute issues
			let (issue_fee_1, issue_fee_2, issue_fee_3) = execute_multiple_issues();

			//nominator reward withdraw
			//NOMINATOR collateral = 500, total collateral for vault 1500
			//we expect balance to increase on asset1 = floor( 170*(500/1500) )
			let vault_1_collateral_usd = to_usd(&collateral_1, &VAULT.collateral_currency());
			let nominator_amount_usd = to_usd(&nominator_amount, &VAULT.collateral_currency());
			let vault_2_collateral_usd = to_usd(&collateral_2, &VAULT_2.collateral_currency());
			let vault_3_collateral_usd = to_usd(&collateral_3, &VAULT_3.collateral_currency());
			let vault_4_collateral_usd = to_usd(&collateral_4, &VAULT_4.collateral_currency());
			let vault_5_collateral_usd = to_usd(&collateral_5, &VAULT_5.collateral_currency());

			let total_amount_usd = vault_1_collateral_usd
				+ nominator_amount_usd
				+ vault_2_collateral_usd
				+ vault_3_collateral_usd
				+ vault_4_collateral_usd
				+ vault_5_collateral_usd;

			let reward_for_pool_1_asset_1 =
				((((vault_1_collateral_usd + vault_2_collateral_usd + nominator_amount_usd)
					as f64 / total_amount_usd as f64)
					* issue_fee_1 as f64)
					.floor() * ((collateral_1 + nominator_amount) as f64
					/ (collateral_1 + collateral_2 + nominator_amount) as f64))
					.floor();

			let reward_for_nominator_asset_1 = (reward_for_pool_1_asset_1
				* (nominator_amount as f64 / (nominator_amount + collateral_1) as f64))
				as u128;

			let reward_for_pool_1_asset_2 =
				((((vault_1_collateral_usd + vault_2_collateral_usd + nominator_amount_usd)
					as f64 / total_amount_usd as f64)
					* issue_fee_2 as f64)
					.floor() * ((collateral_1 + nominator_amount) as f64
					/ (collateral_1 + collateral_2 + nominator_amount) as f64))
					.floor();

			let reward_for_nominator_asset_2 = (reward_for_pool_1_asset_2
				* (nominator_amount as f64 / (nominator_amount + collateral_1) as f64))
				as u128;

			let reward_for_pool_1_asset_3 =
				((((vault_1_collateral_usd + vault_2_collateral_usd + nominator_amount_usd)
					as f64 / total_amount_usd as f64)
					* issue_fee_3 as f64)
					.floor() * ((collateral_1 + nominator_amount) as f64
					/ (collateral_1 + collateral_2 + nominator_amount) as f64))
					.floor();

			let reward_for_nominator_asset_3 = (reward_for_pool_1_asset_3
				* (nominator_amount as f64 / (nominator_amount + collateral_1) as f64))
				as u128;

			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				RuntimeOrigin::signed(NOMINATOR1.clone()).into(),
				VAULT,
				VAULT.wrapped_currency(),
				None,
			));
			assert_eq!(
				get_balance(VAULT.wrapped_currency(), &NOMINATOR1),
				reward_for_nominator_asset_1
			);

			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				RuntimeOrigin::signed(NOMINATOR1.clone()).into(),
				VAULT,
				VAULT_2.wrapped_currency(),
				None,
			));
			assert_eq!(
				get_balance(VAULT_2.wrapped_currency(), &NOMINATOR1),
				reward_for_nominator_asset_2
			);
			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				RuntimeOrigin::signed(NOMINATOR1.clone()).into(),
				VAULT,
				VAULT_3.wrapped_currency(),
				None,
			));
			assert_eq!(
				get_balance(VAULT_3.wrapped_currency(), &NOMINATOR1),
				reward_for_nominator_asset_3
			);
		})
	}

	#[test]
	fn integration_new_nominator_does_not_get_previous_rewards() {
		run_test(|| {
			//ARRANGE
			assert_ok!(Staking::add_reward_currency(VAULT.wrapped_currency()));
			assert_ok!(Staking::add_reward_currency(VAULT_2.wrapped_currency()));
			assert_ok!(Staking::add_reward_currency(VAULT_3.wrapped_currency()));
			//set up the vaults
			let collateral_1: u128 = 1000;
			register_vault_with_collateral(&VAULT, collateral_1);

			let collateral_2: u128 = 2000;
			register_vault_with_collateral(&VAULT_2, collateral_2);

			let collateral_3: u128 = 1500;
			register_vault_with_collateral(&VAULT_3, collateral_3);

			let collateral_4: u128 = 1700;
			register_vault_with_collateral(&VAULT_4, collateral_4);

			let collateral_5: u128 = 800;
			register_vault_with_collateral(&VAULT_5, collateral_5);

			assert_eq!(get_balance(VAULT.wrapped_currency(), &NOMINATOR1), 0);
			assert_eq!(get_balance(VAULT_2.wrapped_currency(), &NOMINATOR1), 0);
			assert_eq!(get_balance(VAULT_3.wrapped_currency(), &NOMINATOR1), 0);
			//execute issues
			//ACT
			let (_issue_fee_1, _issue_fee_2, _issue_fee_3) = execute_multiple_issues();

			//nominate after fee issued!
			let nominator_amount: u128 = 500;
			nominate_vault(NOMINATOR1, VAULT, nominator_amount);

			assert_noop!(
				<reward_distribution::Pallet<Test>>::collect_reward(
					RuntimeOrigin::signed(NOMINATOR1.clone()).into(),
					VAULT,
					VAULT.wrapped_currency(),
					None,
				),
				reward_distribution::Error::<Test>::NoRewardsForAccount
			);

			assert_noop!(
				<reward_distribution::Pallet<Test>>::collect_reward(
					RuntimeOrigin::signed(NOMINATOR1.clone()).into(),
					VAULT,
					VAULT_2.wrapped_currency(),
					None,
				),
				reward_distribution::Error::<Test>::NoRewardsForAccount
			);

			assert_noop!(
				<reward_distribution::Pallet<Test>>::collect_reward(
					RuntimeOrigin::signed(NOMINATOR1.clone()).into(),
					VAULT,
					VAULT_3.wrapped_currency(),
					None,
				),
				reward_distribution::Error::<Test>::NoRewardsForAccount
			);

			//ASSERT
			assert_eq!(get_balance(VAULT.wrapped_currency(), &NOMINATOR1), 0);
			assert_eq!(get_balance(VAULT_2.wrapped_currency(), &NOMINATOR1), 0);
			assert_eq!(get_balance(VAULT_3.wrapped_currency(), &NOMINATOR1), 0);
		})
	}
}
