use frame_support::{assert_noop, assert_ok, dispatch::DispatchError};
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
mod integration_tests {
	use super::*;
	use orml_traits::MultiCurrency;
	use pooled_rewards::RewardsApi;

	fn get_reward_for_vault(vault: &DefaultVaultId<Test>, reward_currency: CurrencyId) -> Balance {
		<<Test as fee::Config>::VaultRewards as RewardsApi<
			CurrencyId,
			DefaultVaultId<Test>,
			Balance,
			CurrencyId,
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
				vault_clone_2.clone().wrapped_currency(),
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
			//set up the vaults
			let collateral1: u128 = 1000;
			register_vault_with_collateral(&VAULT, collateral1);

			let collateral2: u128 = 2000;
			register_vault_with_collateral(&VAULT2, collateral2);

			//execute the issue
			let issue_asset = VAULT.wrapped_currency();
			let issue_amount = 30;
			let issue_fee = 10;
			let griefing_collateral = 1;
			let amount_transferred = 30;
			let issue_id =
				setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred)
					.unwrap();

			assert_ok!(execute_issue(USER, &issue_id));

			//execute the issue on the other currency
			let issue_asset2 = VAULT2.wrapped_currency();
			let issue_amount2 = 30;
			let issue_fee2 = 20;
			let griefing_collateral2 = 1;
			let amount_transferred2 = 30;
			let issue_id2 = setup_execute_with_vault(
				issue_amount2,
				issue_fee2,
				griefing_collateral2,
				amount_transferred2,
				VAULT2.clone(),
			)
			.unwrap();

			assert_ok!(execute_issue(USER, &issue_id2));

			//ensure that the current rewards equal to fee
			//Example: we expect for reward(vault1, asset1) = floor( 10*(1000/3000) ) = 3
			assert_eq!(get_reward_for_vault(&VAULT, issue_asset), 3);
			assert_eq!(get_reward_for_vault(&VAULT2, issue_asset), 6);

			assert_eq!(get_reward_for_vault(&VAULT, issue_asset2), 6);
			assert_eq!(get_reward_for_vault(&VAULT2, issue_asset2), 13);
		})
	}

	fn execute_multiple_issues() {
		//execute the issue
		let issue_amount = 3000;
		let issue_fee = 1000;
		let griefing_collateral = 1;
		let amount_transferred = 3000;
		let issue_id =
			setup_execute(issue_amount, issue_fee, griefing_collateral, amount_transferred)
				.unwrap();
		assert_ok!(execute_issue(USER, &issue_id));

		//execute the issue on the other currency
		let issue_amount2 = 3000;
		let issue_fee2 = 500;
		let griefing_collateral2 = 1;
		let amount_transferred2 = 3000;
		let issue_id2 = setup_execute_with_vault(
			issue_amount2,
			issue_fee2,
			griefing_collateral2,
			amount_transferred2,
			VAULT2.clone(),
		)
		.unwrap();
		assert_ok!(execute_issue(USER, &issue_id2));

		let issue_amount3 = 3000;
		let issue_fee3 = 600;
		let griefing_collateral3 = 1;
		let amount_transferred3 = 3000;
		let issue_id3 = setup_execute_with_vault(
			issue_amount3,
			issue_fee3,
			griefing_collateral3,
			amount_transferred3,
			VAULT3.clone(),
		)
		.unwrap();
		assert_ok!(execute_issue(USER, &issue_id3));

		let issue_amount4 = 3000;
		let issue_fee4 = 400;
		let griefing_collateral4 = 1;
		let amount_transferred4 = 3000;
		let issue_id4 = setup_execute_with_vault(
			issue_amount4,
			issue_fee4,
			griefing_collateral4,
			amount_transferred4,
			VAULT4.clone(),
		)
		.unwrap();
		assert_ok!(execute_issue(USER, &issue_id4));

		let issue_amount5 = 3000;
		let issue_fee5 = 700;
		let griefing_collateral5 = 1;
		let amount_transferred5 = 3000;
		let issue_id5 = setup_execute_with_vault(
			issue_amount5,
			issue_fee5,
			griefing_collateral5,
			amount_transferred5,
			VAULT5.clone(),
		)
		.unwrap();
		assert_ok!(execute_issue(USER, &issue_id5));
	}

	#[test]
	fn integration_multiple_vaults_many_collateral() {
		run_test(|| {
			//set up the vaults
			let collateral1: u128 = 1000;
			register_vault_with_collateral(&VAULT, collateral1);

			let collateral2: u128 = 2000;
			register_vault_with_collateral(&VAULT2, collateral2);

			let collateral3: u128 = 1500;
			register_vault_with_collateral(&VAULT3, collateral3);

			let collateral4: u128 = 1700;
			register_vault_with_collateral(&VAULT4, collateral4);

			let collateral5: u128 = 800;
			register_vault_with_collateral(&VAULT5, collateral5);

			//execute issues
			let issue_asset = VAULT.wrapped_currency();
			let issue_asset2 = VAULT2.wrapped_currency();
			let issue_asset3 = VAULT3.wrapped_currency();
			execute_multiple_issues();

			//ensure that the current rewards equal to fee
			//Example: we expect for reward(vault1, asset1) = floor(
			//floor((1000/3000)*floor( 1400*(3000*5/(3000*5+3200*10+800*15)) )) = 118

			//assertions for asset1 = asset4
			assert_eq!(get_reward_for_vault(&VAULT, issue_asset), 118);
			//NOTE: withouth floor(), should actually exactly be 237.25. In this case
			//it is 236. The discrepancy is due to two consecutive roundings
			assert_eq!(get_reward_for_vault(&VAULT2, issue_asset), 236);
			assert_eq!(get_reward_for_vault(&VAULT3, issue_asset), 355);
			assert_eq!(get_reward_for_vault(&VAULT4, issue_asset), 402);
			assert_eq!(get_reward_for_vault(&VAULT5, issue_asset), 284);

			//assertions for issue asset2 = asset5
			assert_eq!(get_reward_for_vault(&VAULT, issue_asset2), 101);
			assert_eq!(get_reward_for_vault(&VAULT2, issue_asset2), 202);
			assert_eq!(get_reward_for_vault(&VAULT3, issue_asset2), 304);
			assert_eq!(get_reward_for_vault(&VAULT4, issue_asset2), 345);
			assert_eq!(get_reward_for_vault(&VAULT5, issue_asset2), 243);

			//assertions for issue asset3
			assert_eq!(get_reward_for_vault(&VAULT, issue_asset3), 50);
			assert_eq!(get_reward_for_vault(&VAULT2, issue_asset3), 101);
			assert_eq!(get_reward_for_vault(&VAULT3, issue_asset3), 152);
			assert_eq!(get_reward_for_vault(&VAULT4, issue_asset3), 172);
			assert_eq!(get_reward_for_vault(&VAULT5, issue_asset3), 122);
		});
	}

	//same issue and fee set up as previous test, but nominators are
	//taken into account.
	//only vault 1 is taken into account for testing nominator rewards
	#[test]
	fn integration_multiple_vaults_many_collateral_nominated() {
		run_test(|| {
			//set up the vaults
			let collateral1: u128 = 1000;
			register_vault_with_collateral(&VAULT, collateral1);

			let collateral2: u128 = 2000;
			register_vault_with_collateral(&VAULT2, collateral2);

			let collateral3: u128 = 1500;
			register_vault_with_collateral(&VAULT3, collateral3);

			let collateral4: u128 = 1700;
			register_vault_with_collateral(&VAULT4, collateral4);

			let collateral5: u128 = 800;
			register_vault_with_collateral(&VAULT5, collateral5);

			//nominate
			let nominator_amount: u128 = 500;

			nominate_vault(NOMINATOR1, VAULT, nominator_amount);

			//execute issues
			execute_multiple_issues();

			//assertions for asset1 = asset4
			assert_eq!(get_reward_for_vault(&VAULT, VAULT.wrapped_currency()), 170);

			//assertions for issue asset2 = asset5
			assert_eq!(get_reward_for_vault(&VAULT, VAULT2.wrapped_currency()), 146);

			//assertions for issue asset3
			assert_eq!(get_reward_for_vault(&VAULT, VAULT3.wrapped_currency()), 72);

			//nominator reward withdraw
			//NOMINATOR collateral = 500, total collateral for vault 1500
			//we expect balance to increase on asset1 = floor( 170*(500/1500) )
			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				RuntimeOrigin::signed(NOMINATOR1.clone()).into(),
				VAULT,
				VAULT.wrapped_currency(),
				None,
			));
			assert_eq!(get_balance(VAULT.wrapped_currency(), &NOMINATOR1), 56);
		})
	}
}
