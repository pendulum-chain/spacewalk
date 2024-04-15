use codec::Decode;
use currency::{testing_constants::DEFAULT_WRAPPED_CURRENCY, Amount};
use frame_support::{assert_err, assert_noop, assert_ok, error::BadOrigin};
use frame_system::RawOrigin;
use mocktopus::mocking::*;
use pooled_rewards::RewardsApi;
use pretty_assertions::assert_eq;
use primitives::{DecimalsLookup, StellarPublicKeyRaw, VaultCurrencyPair, VaultId};
use security::Pallet as Security;
use sp_arithmetic::{traits::One, FixedPointNumber, FixedU128};
use sp_core::U256;
use sp_runtime::{
	offchain::{testing::TestTransactionPoolExt, TransactionPoolExt},
	ArithmeticError,
};
use sp_std::convert::TryInto;

use crate::{
	ext,
	mock::*,
	types::{BalanceOf, UpdatableVault},
	CurrencySource, DefaultVaultId, DispatchError, PunishmentDelay, Vault, VaultStatus,
};

type Event = crate::Event<Test>;

const STELLAR_PUBLIC_KEY_DUMMY: StellarPublicKeyRaw = [1u8; 32];

fn vault_id(account_id: AccountId) -> VaultId<AccountId, CurrencyId> {
	VaultId {
		account_id,
		currencies: VaultCurrencyPair {
			collateral: DEFAULT_COLLATERAL_CURRENCY,
			wrapped: DEFAULT_WRAPPED_CURRENCY,
		},
	}
}

// use macro to avoid messing up stack trace
macro_rules! assert_emitted {
	($event:expr) => {
		let test_event = TestEvent::VaultRegistry($event);
		assert!(System::events().iter().any(|a| a.event == test_event));
	};
	($event:expr, $times:expr) => {
		let test_event = TestEvent::VaultRegistry($event);
		assert_eq!(System::events().iter().filter(|a| a.event == test_event).count(), $times);
	};
}

macro_rules! assert_not_emitted {
	($event:expr) => {
		let test_event = TestEvent::VaultRegistry($event);
		assert!(!System::events().iter().any(|a| a.event == test_event));
	};
}

fn convert_with_exchange_rate(
	exchange_rate: u128,
) -> impl Fn(
	CurrencyId,
	Amount<Test>,
) -> MockResult<(CurrencyId, Amount<Test>), Result<Amount<Test>, DispatchError>> {
	move |currency_id, amount| {
		let amount = if currency_id == DEFAULT_WRAPPED_CURRENCY {
			Amount::new(amount.amount() / exchange_rate, currency_id)
		} else {
			Amount::new(amount.amount() * exchange_rate, currency_id)
		};
		MockResult::Return(Ok(amount))
	}
}

fn create_vault_with_collateral(id: &DefaultVaultId<Test>, collateral: u128) {
	VaultRegistry::get_minimum_collateral_vault
		.mock_safe(move |currency_id| MockResult::Return(Amount::new(collateral, currency_id)));
	let origin = RuntimeOrigin::signed(id.account_id);

	assert_ok!(VaultRegistry::register_public_key(origin.clone(), STELLAR_PUBLIC_KEY_DUMMY));
	assert_ok!(VaultRegistry::register_vault(origin, id.currencies.clone(), collateral));
}

fn create_vault(id: DefaultVaultId<Test>) -> DefaultVaultId<Test> {
	create_vault_with_collateral(&id, DEFAULT_COLLATERAL);
	id
}

fn create_sample_vault() -> DefaultVaultId<Test> {
	create_vault(COLLATERAL_1_VAULT_1)
}

fn amount(amount: u128) -> Amount<Test> {
	Amount::new(amount, DEFAULT_COLLATERAL_CURRENCY)
}

fn griefing(amount: u128) -> Amount<Test> {
	Amount::new(amount, DEFAULT_NATIVE_CURRENCY)
}

fn wrapped(amount: u128) -> Amount<Test> {
	Amount::new(amount, DEFAULT_WRAPPED_CURRENCY)
}

fn create_vault_and_issue_tokens(
	issue_tokens: u128,
	collateral: u128,
	id: DefaultVaultId<Test>,
) -> DefaultVaultId<Test> {
	// vault has no tokens issued yet
	create_vault_with_collateral(&id, collateral);

	// exchange rate 1 Satoshi = 10 Planck (smallest unit of DOT)
	convert_to.mock_safe(move |currency, x| {
		MockResult::Return(Ok(Amount::new(x.amount() / 10, currency)))
	});

	// issue tokens with 200% collateralization of DEFAULT_COLLATERAL
	assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(issue_tokens)));
	let res = VaultRegistry::issue_tokens(&id, &wrapped(issue_tokens));
	assert_ok!(res);

	// mint tokens to the vault
	let amount = Amount::<Test>::new(issue_tokens, DEFAULT_WRAPPED_CURRENCY);
	amount.mint_to(&id.account_id).unwrap();

	id
}

fn create_sample_vault_and_issue_tokens(issue_tokens: u128) -> DefaultVaultId<Test> {
	create_vault_and_issue_tokens(issue_tokens, DEFAULT_COLLATERAL, COLLATERAL_1_VAULT_1)
}

#[test]
fn set_punishment_delay_works() {
	run_test(|| {
		let punishment_delay = 99;
		assert_ok!(VaultRegistry::set_punishment_delay(RuntimeOrigin::root(), punishment_delay));
		assert_eq!(PunishmentDelay::<Test>::get(), punishment_delay);
	})
}

#[test]
fn set_punishment_delay_fails_for_wrong_origin() {
	run_test(|| {
		let punishment_delay = 99;
		assert_err!(
			VaultRegistry::set_punishment_delay(RuntimeOrigin::signed(1), punishment_delay),
			BadOrigin
		);
	})
}

#[test]
fn register_vault_succeeds() {
	run_test(|| {
		let id = create_sample_vault();
		assert_emitted!(Event::RegisterVault { vault_id: id, collateral: DEFAULT_COLLATERAL });
	});
}

#[test]
fn registering_public_key_twice_fails() {
	run_test(|| {
		let origin = RuntimeOrigin::signed(COLLATERAL_1_VAULT_1.account_id);
		let public_key_1: StellarPublicKeyRaw = [0u8; 32];
		let public_key_2: StellarPublicKeyRaw = [1u8; 32];
		assert_ok!(VaultRegistry::register_public_key(origin.clone(), public_key_1));
		assert_err!(
			VaultRegistry::register_public_key(origin, public_key_2),
			TestError::PublicKeyAlreadyRegistered
		);
	})
}

#[test]
fn register_vault_fails_when_given_collateral_too_low() {
	run_test(|| {
		VaultRegistry::get_minimum_collateral_vault
			.mock_safe(move |currency_id| MockResult::Return(Amount::new(200, currency_id)));
		let id = COLLATERAL_1_VAULT_1;
		let collateral = 100;

		let origin = RuntimeOrigin::signed(id.account_id);
		assert_ok!(VaultRegistry::register_public_key(origin.clone(), STELLAR_PUBLIC_KEY_DUMMY));

		let result = VaultRegistry::register_vault(origin, id.currencies.clone(), collateral);
		assert_err!(result, TestError::InsufficientVaultCollateralAmount);
		assert_not_emitted!(Event::RegisterVault { vault_id: id, collateral });
	});
}

#[test]
fn register_vault_fails_when_account_funds_too_low() {
	run_test(|| {
		let collateral = DEFAULT_COLLATERAL + 1;

		let origin = RuntimeOrigin::signed(COLLATERAL_1_VAULT_1.account_id);
		assert_ok!(VaultRegistry::register_public_key(origin.clone(), STELLAR_PUBLIC_KEY_DUMMY));

		let result =
			VaultRegistry::register_vault(origin, COLLATERAL_1_VAULT_1.currencies, collateral);
		assert_err!(result, TokensError::BalanceTooLow);
		assert_not_emitted!(Event::RegisterVault { vault_id: COLLATERAL_1_VAULT_1, collateral });
	});
}

#[test]
fn register_vault_fails_when_already_registered() {
	run_test(|| {
		let id = create_sample_vault();
		let result = VaultRegistry::register_vault(
			RuntimeOrigin::signed(id.account_id),
			DEFAULT_CURRENCY_PAIR,
			DEFAULT_COLLATERAL,
		);
		assert_err!(result, TestError::VaultAlreadyRegistered);
		assert_emitted!(Event::RegisterVault { vault_id: id, collateral: DEFAULT_COLLATERAL }, 1);
	});
}

#[test]
fn deposit_collateral_succeeds() {
	run_test(|| {
		let id = create_vault(RICH_ID);
		let additional = RICH_COLLATERAL - DEFAULT_COLLATERAL;
		let res = VaultRegistry::deposit_collateral(
			RuntimeOrigin::signed(id.account_id),
			DEFAULT_CURRENCY_PAIR,
			additional,
		);
		assert_ok!(res);
		let new_collateral = ext::currency::get_reserved_balance::<Test>(
			DEFAULT_COLLATERAL_CURRENCY,
			&id.account_id,
		);
		assert_eq!(new_collateral, amount(DEFAULT_COLLATERAL + additional));
		assert_emitted!(Event::DepositCollateral {
			vault_id: id,
			new_collateral: additional,
			total_collateral: RICH_COLLATERAL,
			free_collateral: RICH_COLLATERAL
		});
	});
}

#[test]
fn deposit_collateral_fails_when_vault_does_not_exist() {
	run_test(|| {
		let res =
			VaultRegistry::deposit_collateral(RuntimeOrigin::signed(3), DEFAULT_CURRENCY_PAIR, 50);
		assert_err!(res, TestError::VaultNotFound);
	})
}

#[test]
fn withdraw_collateral_succeeds() {
	run_test(|| {
		let id = create_sample_vault();
		let res = VaultRegistry::withdraw_collateral(
			RuntimeOrigin::signed(id.account_id),
			id.currencies.clone(),
			50,
		);
		assert_ok!(res);
		let new_collateral = ext::currency::get_reserved_balance::<Test>(
			DEFAULT_COLLATERAL_CURRENCY,
			&id.account_id,
		);
		assert_eq!(new_collateral, amount(DEFAULT_COLLATERAL - 50));
		assert_emitted!(Event::WithdrawCollateral {
			vault_id: id,
			withdrawn_amount: 50,
			total_collateral: DEFAULT_COLLATERAL - 50
		});
	});
}

#[test]
fn withdraw_collateral_fails_when_vault_does_not_exist() {
	run_test(|| {
		let res =
			VaultRegistry::withdraw_collateral(RuntimeOrigin::signed(3), DEFAULT_CURRENCY_PAIR, 50);
		assert_err!(res, TestError::VaultNotFound);
	})
}

#[test]
fn withdraw_collateral_fails_when_not_enough_collateral() {
	run_test(|| {
		let id = create_sample_vault();
		let res = VaultRegistry::withdraw_collateral(
			RuntimeOrigin::signed(id.account_id),
			id.currencies,
			DEFAULT_COLLATERAL + 1,
		);
		assert_err!(res, TestError::InsufficientCollateral);
	})
}

#[test]
fn try_increase_to_be_issued_tokens_succeeds() {
	run_test(|| {
		let id = create_sample_vault();
		let res = VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50));
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_ok!(res);
		assert_eq!(vault.data.to_be_issued_tokens, 50);
		assert_emitted!(Event::IncreaseToBeIssuedTokens { vault_id: id, increase: 50 });
	});
}

#[test]
fn try_increase_to_be_issued_tokens_fails_with_insufficient_collateral() {
	run_test(|| {
		let id = create_sample_vault();
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		let res = VaultRegistry::try_increase_to_be_issued_tokens(
			&id,
			&wrapped(vault.issuable_tokens().unwrap().amount() + 1),
		);
		// important: should not change the storage state
		assert_noop!(res, TestError::ExceedingVaultLimit);
	});
}

#[test]
fn decrease_to_be_issued_tokens_succeeds() {
	run_test(|| {
		let id = create_sample_vault();
		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)),);
		let res = VaultRegistry::decrease_to_be_issued_tokens(&id, &wrapped(50));
		assert_ok!(res);
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_eq!(vault.data.to_be_issued_tokens, 0);
		assert_emitted!(Event::DecreaseToBeIssuedTokens { vault_id: id, decrease: 50 });
	});
}

#[test]
fn decrease_to_be_issued_tokens_fails_with_insufficient_tokens() {
	run_test(|| {
		let id = create_sample_vault();

		let res = VaultRegistry::decrease_to_be_issued_tokens(&id, &wrapped(50));
		assert_err!(res, ArithmeticError::Underflow);
	});
}

#[test]
fn issue_tokens_succeeds() {
	run_test(|| {
		let id = create_sample_vault();
		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)),);
		let res = VaultRegistry::issue_tokens(&id, &wrapped(50));
		assert_ok!(res);
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_eq!(vault.data.to_be_issued_tokens, 0);
		assert_eq!(vault.data.issued_tokens, 50);
		assert_emitted!(Event::IssueTokens { vault_id: id, increase: 50 });
	});
}

#[test]
fn issue_tokens_fails_with_insufficient_tokens() {
	run_test(|| {
		let id = create_sample_vault();

		assert_err!(VaultRegistry::issue_tokens(&id, &wrapped(50)), ArithmeticError::Underflow);
	});
}

#[test]
fn try_increase_to_be_replaced_tokens_succeeds() {
	run_test(|| {
		let id = create_sample_vault();

		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)),);
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(50)));
		assert_ok!(VaultRegistry::try_increase_to_be_replaced_tokens(&id, &wrapped(50)));
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_eq!(vault.data.issued_tokens, 50);
		assert_eq!(vault.data.to_be_replaced_tokens, 50);
		assert_emitted!(Event::IncreaseToBeReplacedTokens { vault_id: id, increase: 50 });
	});
}

#[test]
fn try_increase_to_be_replaced_tokens_fails_with_insufficient_tokens() {
	run_test(|| {
		let id = create_sample_vault();

		// important: should not change the storage state
		assert_noop!(
			VaultRegistry::try_increase_to_be_replaced_tokens(&id, &wrapped(50)),
			TestError::InsufficientTokensCommitted
		);
	});
}

#[test]
fn try_increase_to_be_redeemed_tokens_succeeds() {
	run_test(|| {
		let id = create_sample_vault();

		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)),);
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(50)));
		let res = VaultRegistry::try_increase_to_be_redeemed_tokens(&id, &wrapped(50));
		assert_ok!(res);
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_eq!(vault.data.issued_tokens, 50);
		assert_eq!(vault.data.to_be_redeemed_tokens, 50);
		assert_emitted!(Event::IncreaseToBeRedeemedTokens { vault_id: id, increase: 50 });
	});
}

#[test]
fn try_increase_to_be_redeemed_tokens_fails_with_insufficient_tokens() {
	run_test(|| {
		let id = create_sample_vault();

		let res = VaultRegistry::try_increase_to_be_redeemed_tokens(&id, &wrapped(50));

		// important: should not change the storage state
		assert_noop!(res, TestError::InsufficientTokensCommitted);
	});
}

#[test]
fn decrease_to_be_redeemed_tokens_succeeds() {
	run_test(|| {
		let id = create_sample_vault();
		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)),);
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(50)));
		assert_ok!(VaultRegistry::try_increase_to_be_redeemed_tokens(&id, &wrapped(50)));
		let res = VaultRegistry::decrease_to_be_redeemed_tokens(&id, &wrapped(50));
		assert_ok!(res);
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_eq!(vault.data.issued_tokens, 50);
		assert_eq!(vault.data.to_be_redeemed_tokens, 0);
		assert_emitted!(Event::DecreaseToBeRedeemedTokens { vault_id: id, decrease: 50 });
	});
}

#[test]
fn decrease_to_be_redeemed_tokens_fails_with_insufficient_tokens() {
	run_test(|| {
		let id = create_sample_vault();

		let res = VaultRegistry::decrease_to_be_redeemed_tokens(&id, &wrapped(50));
		assert_err!(res, ArithmeticError::Underflow);
	});
}

#[test]
fn decrease_tokens_succeeds() {
	run_test(|| {
		let id = create_sample_vault();
		let user_id = 5;
		VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)).unwrap();
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(50)));
		assert_ok!(VaultRegistry::try_increase_to_be_redeemed_tokens(&id, &wrapped(50)));
		let res = VaultRegistry::decrease_tokens(&id, &user_id, &wrapped(50));
		assert_ok!(res);
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_eq!(vault.data.issued_tokens, 0);
		assert_eq!(vault.data.to_be_redeemed_tokens, 0);
		assert_emitted!(Event::DecreaseTokens { vault_id: id, user_id, decrease: 50 });
	});
}

#[test]
fn decrease_tokens_fails_with_insufficient_tokens() {
	run_test(|| {
		let id = create_sample_vault();
		let user_id = 5;
		VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)).unwrap();
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(50)));
		let res = VaultRegistry::decrease_tokens(&id, &user_id, &wrapped(50));
		assert_err!(res, ArithmeticError::Underflow);
	});
}

#[test]
fn redeem_tokens_succeeds() {
	run_test(|| {
		let id = create_sample_vault();
		VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)).unwrap();
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(50)));
		assert_ok!(VaultRegistry::try_increase_to_be_redeemed_tokens(&id, &wrapped(50)));
		let res = VaultRegistry::redeem_tokens(&id, &wrapped(50), &amount(0), &0);
		assert_ok!(res);
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_eq!(vault.data.issued_tokens, 0);
		assert_eq!(vault.data.to_be_redeemed_tokens, 0);
		assert_emitted!(Event::RedeemTokens { vault_id: id, redeemed_amount: 50 });
	});
}

#[test]
fn redeem_tokens_fails_with_insufficient_tokens() {
	run_test(|| {
		let id = create_sample_vault();
		VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)).unwrap();
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(50)));
		let res = VaultRegistry::redeem_tokens(&id, &wrapped(50), &amount(0), &0);
		assert_err!(res, ArithmeticError::Underflow);
	});
}

#[test]
fn redeem_tokens_premium_succeeds() {
	run_test(|| {
		let id = create_sample_vault();
		let user_id = 5;
		// TODO: emulate assert_called
		let id_copy = id.clone();
		VaultRegistry::transfer_funds.mock_safe(move |sender, receiver, _amount| {
			assert_eq!(sender, CurrencySource::Collateral(id_copy.clone()));
			assert_eq!(receiver, CurrencySource::FreeBalance(user_id));
			MockResult::Return(Ok(()))
		});

		VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)).unwrap();
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(50)));
		assert_ok!(VaultRegistry::try_increase_to_be_redeemed_tokens(&id, &wrapped(50)));
		assert_ok!(VaultRegistry::redeem_tokens(&id, &wrapped(50), &amount(30), &user_id));

		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_eq!(vault.data.issued_tokens, 0);
		assert_eq!(vault.data.to_be_redeemed_tokens, 0);
		assert_emitted!(Event::RedeemTokensPremium {
			vault_id: id,
			redeemed_amount: 50,
			collateral: 30,
			user_id
		});
	});
}

#[test]
fn redeem_tokens_premium_fails_with_insufficient_tokens() {
	run_test(|| {
		let id = create_sample_vault();
		let user_id = 5;
		VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(50)).unwrap();
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(50)));
		let res = VaultRegistry::redeem_tokens(&id, &wrapped(50), &amount(30), &user_id);
		assert_err!(res, ArithmeticError::Underflow);
		assert_not_emitted!(Event::RedeemTokensPremium {
			vault_id: id,
			redeemed_amount: 50,
			collateral: 30,
			user_id
		});
	});
}

#[test]
fn redeem_tokens_liquidation_succeeds() {
	run_test(|| {
		let mut liquidation_vault =
			VaultRegistry::get_rich_liquidation_vault(&DEFAULT_CURRENCY_PAIR);
		let user_id = 5;

		// TODO: emulate assert_called
		VaultRegistry::transfer_funds.mock_safe(move |sender, receiver, _amount| {
			assert_eq!(sender, CurrencySource::LiquidationVault(DEFAULT_CURRENCY_PAIR));
			assert_eq!(receiver, CurrencySource::FreeBalance(user_id));
			MockResult::Return(Ok(()))
		});

		// liquidation vault collateral
		crate::types::CurrencySource::<Test>::current_balance
			.mock_safe(|_, _| MockResult::Return(Ok(amount(1000))));

		assert_ok!(liquidation_vault.increase_to_be_issued(&wrapped(50)));
		assert_ok!(liquidation_vault.increase_issued(&wrapped(50)));

		assert_ok!(VaultRegistry::redeem_tokens_liquidation(
			DEFAULT_COLLATERAL_CURRENCY,
			&user_id,
			&wrapped(50)
		));
		let liquidation_vault = VaultRegistry::get_rich_liquidation_vault(&DEFAULT_CURRENCY_PAIR);
		assert_eq!(liquidation_vault.data.issued_tokens, 0);
		assert_emitted!(Event::RedeemTokensLiquidation {
			redeemer_id: user_id,
			burned_tokens: 50,
			transferred_collateral: 500
		});
	});
}

#[test]
fn redeem_tokens_liquidation_does_not_call_recover_when_unnecessary() {
	run_test(|| {
		let mut liquidation_vault =
			VaultRegistry::get_rich_liquidation_vault(&DEFAULT_CURRENCY_PAIR);
		let user_id = 5;

		VaultRegistry::transfer_funds.mock_safe(move |sender, receiver, _amount| {
			assert_eq!(sender, CurrencySource::LiquidationVault(DEFAULT_CURRENCY_PAIR));
			assert_eq!(receiver, CurrencySource::FreeBalance(user_id));
			MockResult::Return(Ok(()))
		});

		// liquidation vault collateral
		crate::types::CurrencySource::<Test>::current_balance
			.mock_safe(|_, _| MockResult::Return(Ok(amount(1000))));

		assert_ok!(liquidation_vault.increase_to_be_issued(&wrapped(25)));
		assert_ok!(liquidation_vault.increase_issued(&wrapped(25)));

		assert_ok!(VaultRegistry::redeem_tokens_liquidation(
			DEFAULT_COLLATERAL_CURRENCY,
			&user_id,
			&wrapped(10)
		));
		let liquidation_vault = VaultRegistry::get_rich_liquidation_vault(&DEFAULT_CURRENCY_PAIR);
		assert_eq!(liquidation_vault.data.issued_tokens, 15);
		assert_emitted!(Event::RedeemTokensLiquidation {
			redeemer_id: user_id,
			burned_tokens: 10,
			transferred_collateral: (1000 * 10) / 50
		});
	});
}

#[test]
fn redeem_tokens_liquidation_fails_with_insufficient_tokens() {
	run_test(|| {
		let user_id = 5;
		let res = VaultRegistry::redeem_tokens_liquidation(
			DEFAULT_COLLATERAL_CURRENCY,
			&user_id,
			&wrapped(50),
		);
		assert_err!(res, TestError::InsufficientTokensCommitted);
		assert_not_emitted!(Event::RedeemTokensLiquidation {
			redeemer_id: user_id,
			burned_tokens: 50,
			transferred_collateral: 50
		});
	});
}

#[test]
fn replace_tokens_liquidation_succeeds() {
	run_test(|| {
		let old_id = create_sample_vault();
		let new_id = create_vault(COLLATERAL_1_VAULT_2);
		// let new_id_copy = new_id.clone();

		let new_id_copy = new_id.clone();
		currency::Amount::<Test>::lock_on.mock_safe(move |amount, sender| {
			assert_eq!(sender, &new_id_copy.account_id);
			assert_eq!(amount.amount(), 20);
			MockResult::Return(Ok(()))
		});
		VaultRegistry::try_increase_to_be_issued_tokens(&old_id, &wrapped(50)).unwrap();
		assert_ok!(VaultRegistry::issue_tokens(&old_id, &wrapped(50)));
		assert_ok!(VaultRegistry::try_increase_to_be_redeemed_tokens(&old_id, &wrapped(50)));
		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&new_id, &wrapped(50)));

		assert_ok!(VaultRegistry::replace_tokens(&old_id, &new_id, &wrapped(50), &amount(20)));

		let old_vault = VaultRegistry::get_active_rich_vault_from_id(&old_id).unwrap();
		let new_vault = VaultRegistry::get_active_rich_vault_from_id(&new_id).unwrap();
		assert_eq!(old_vault.data.issued_tokens, 0);
		assert_eq!(old_vault.data.to_be_redeemed_tokens, 0);
		assert_eq!(new_vault.data.issued_tokens, 50);
		assert_eq!(new_vault.data.to_be_issued_tokens, 0);
		assert_emitted!(Event::ReplaceTokens {
			old_vault_id: old_id,
			new_vault_id: new_id,
			amount: 50,
			additional_collateral: 20
		});
	});
}

#[test]
fn cancel_replace_tokens_succeeds() {
	run_test(|| {
		let old_id = create_sample_vault();
		let new_id = create_vault(COLLATERAL_1_VAULT_2);

		let new_id_copy = new_id.clone();
		currency::Amount::<Test>::lock_on.mock_safe(move |amount, sender| {
			assert_eq!(sender, &new_id_copy.account_id);
			assert_eq!(amount.amount(), 20);
			MockResult::Return(Ok(()))
		});

		VaultRegistry::try_increase_to_be_issued_tokens(&old_id, &wrapped(50)).unwrap();
		assert_ok!(VaultRegistry::issue_tokens(&old_id, &wrapped(50)));
		assert_ok!(VaultRegistry::try_increase_to_be_redeemed_tokens(&old_id, &wrapped(50)));
		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&new_id, &wrapped(50)));

		assert_ok!(VaultRegistry::cancel_replace_tokens(&old_id, &new_id, &wrapped(50)));

		let old_vault = VaultRegistry::get_active_rich_vault_from_id(&old_id).unwrap();
		let new_vault = VaultRegistry::get_active_rich_vault_from_id(&new_id).unwrap();
		assert_eq!(old_vault.data.issued_tokens, 50);
		assert_eq!(old_vault.data.to_be_redeemed_tokens, 0);
		assert_eq!(new_vault.data.issued_tokens, 0);
		assert_eq!(new_vault.data.to_be_issued_tokens, 0);
	});
}

#[test]
fn liquidate_at_most_liquidation_threshold() {
	run_test(|| {
		let vault_id = COLLATERAL_1_VAULT_1;

		let issued_tokens = 100;
		let to_be_issued_tokens = 25;
		let to_be_redeemed_tokens = 40;

		let backing_collateral = 10000;
		let liquidated_collateral = 1875; // 125 * 10 * 1.5
		let liquidated_for_issued = 1275; // 125 * 10 * 1.5

		create_vault_with_collateral(&vault_id, backing_collateral);
		let liquidation_vault_before =
			VaultRegistry::get_rich_liquidation_vault(&DEFAULT_CURRENCY_PAIR);

		VaultRegistry::_set_liquidation_collateral_threshold(
			DEFAULT_CURRENCY_PAIR,
			FixedU128::checked_from_rational(150, 100).unwrap(), // 150%
		);

		VaultRegistry::_set_premium_redeem_threshold(
			DEFAULT_CURRENCY_PAIR,
			FixedU128::checked_from_rational(175, 100).unwrap(), // 175%
		);

		VaultRegistry::_set_secure_collateral_threshold(
			DEFAULT_CURRENCY_PAIR,
			FixedU128::checked_from_rational(200, 100).unwrap(), // 200%
		);

		let collateral_before = ext::currency::get_reserved_balance::<Test>(
			DEFAULT_COLLATERAL_CURRENCY,
			&vault_id.account_id,
		);
		assert_eq!(collateral_before, amount(backing_collateral)); // sanity check

		// required for `issue_tokens` to work
		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(
			&vault_id,
			&wrapped(issued_tokens)
		));
		assert_ok!(VaultRegistry::issue_tokens(&vault_id, &wrapped(issued_tokens)));
		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(
			&vault_id,
			&wrapped(to_be_issued_tokens)
		));
		assert_ok!(VaultRegistry::try_increase_to_be_redeemed_tokens(
			&vault_id,
			&wrapped(to_be_redeemed_tokens)
		));

		let vault_orig = <crate::Vaults<Test>>::get(&vault_id).unwrap();
		let backing_collateral_orig = VaultRegistry::get_backing_collateral(&vault_id).unwrap();

		// set exchange rate
		convert_to.mock_safe(convert_with_exchange_rate(10));

		assert_ok!(VaultRegistry::liquidate_vault(&vault_id));

		// should only be able to withdraw excess above secure threshold
		assert_err!(
			VaultRegistry::withdraw_collateral(
				RuntimeOrigin::signed(vault_id.account_id),
				vault_id.currencies.clone(),
				backing_collateral
			),
			TestError::InsufficientCollateral
		);
		assert_ok!(VaultRegistry::withdraw_collateral(
			RuntimeOrigin::signed(vault_id.account_id),
			vault_id.currencies.clone(),
			backing_collateral - liquidated_collateral
		));

		let liquidation_vault_after =
			VaultRegistry::get_rich_liquidation_vault(&DEFAULT_CURRENCY_PAIR);
		let liquidated_vault = <crate::Vaults<Test>>::get(&vault_id).unwrap();
		assert!(matches!(liquidated_vault.status, VaultStatus::Liquidated));
		assert_emitted!(Event::LiquidateVault {
			vault_id: vault_id.clone(),
			issued_tokens: vault_orig.issued_tokens,
			to_be_issued_tokens: vault_orig.to_be_issued_tokens,
			to_be_redeemed_tokens: vault_orig.to_be_redeemed_tokens,
			to_be_replaced_tokens: vault_orig.to_be_replaced_tokens,
			backing_collateral: backing_collateral_orig.amount(),
			status: VaultStatus::Liquidated,
			replace_collateral: vault_orig.replace_collateral,
		});

		// check liquidation_vault tokens & collateral
		assert_eq!(
			liquidation_vault_after.data.issued_tokens,
			liquidation_vault_before.data.issued_tokens + issued_tokens
		);
		assert_eq!(
			liquidation_vault_after.data.to_be_issued_tokens,
			liquidation_vault_before.data.to_be_issued_tokens + to_be_issued_tokens
		);
		assert_eq!(
			liquidation_vault_after.data.to_be_redeemed_tokens,
			liquidation_vault_before.data.to_be_redeemed_tokens + to_be_redeemed_tokens
		);
		assert_eq!(
			ext::currency::get_reserved_balance::<Test>(
				DEFAULT_COLLATERAL_CURRENCY,
				&liquidation_vault_before.id().account_id
			),
			amount(liquidated_for_issued)
		);

		// check vault tokens & collateral
		let user_vault_after = VaultRegistry::get_rich_vault_from_id(&vault_id).unwrap();
		assert_eq!(user_vault_after.data.issued_tokens, 0);
		assert_eq!(user_vault_after.data.to_be_issued_tokens, 0);
		assert_eq!(user_vault_after.data.to_be_redeemed_tokens, to_be_redeemed_tokens);
		assert_eq!(
			ext::currency::get_reserved_balance::<Test>(
				DEFAULT_COLLATERAL_CURRENCY,
				&vault_id.account_id
			),
			amount(liquidated_collateral - liquidated_for_issued)
		);
		assert_eq!(
			ext::currency::get_free_balance::<Test>(
				DEFAULT_COLLATERAL_CURRENCY,
				&vault_id.account_id
			),
			amount(DEFAULT_COLLATERAL - liquidated_collateral)
		);
	});
}

#[test]
fn can_withdraw_only_up_to_custom_threshold() {
	run_test(|| {
		let vault_id = COLLATERAL_1_VAULT_1;

		let issued_tokens = 100;
		let exchange_rate = 10;
		let secure_threshold = 200u32; // %
		let custom_threshold = 300u32; // %
		let minimum_backing_collateral = 100 * 10 * 2;
		let backing_collateral = 100 * 10 * 3 + 1;

		create_vault_with_collateral(&vault_id, backing_collateral);

		VaultRegistry::_set_secure_collateral_threshold(
			DEFAULT_CURRENCY_PAIR,
			FixedU128::checked_from_rational(secure_threshold, 100).unwrap(), // 200%
		);

		// set custom threshold for vault
		assert_ok!(VaultRegistry::set_custom_secure_threshold(
			RuntimeOrigin::signed(vault_id.account_id),
			DEFAULT_CURRENCY_PAIR,
			UnsignedFixedPoint::checked_from_rational(custom_threshold, 100),
		));

		// set exchange rate
		convert_to.mock_safe(convert_with_exchange_rate(exchange_rate));

		// issue
		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(
			&vault_id,
			&wrapped(issued_tokens)
		));
		assert_ok!(VaultRegistry::issue_tokens(&vault_id, &wrapped(issued_tokens)));

		// should not be able to withdraw below custom threshold
		assert_err!(
			VaultRegistry::withdraw_collateral(
				RuntimeOrigin::signed(vault_id.account_id),
				vault_id.currencies.clone(),
				2
			),
			TestError::InsufficientCollateral
		);
		// should be able to withdraw up to custom threshold
		assert_ok!(VaultRegistry::withdraw_collateral(
			RuntimeOrigin::signed(vault_id.account_id),
			vault_id.currencies.clone(),
			1
		));

		// reset custom threshold
		assert_ok!(VaultRegistry::set_custom_secure_threshold(
			RuntimeOrigin::signed(vault_id.account_id),
			DEFAULT_CURRENCY_PAIR,
			None
		));

		// should not be able to withdraw below base secure threshold now
		assert_err!(
			VaultRegistry::withdraw_collateral(
				RuntimeOrigin::signed(vault_id.account_id),
				vault_id.currencies.clone(),
				backing_collateral - minimum_backing_collateral // will be 1 over
			),
			TestError::InsufficientCollateral
		);

		// should be able to withdraw to the secure threshold
		assert_ok!(VaultRegistry::withdraw_collateral(
			RuntimeOrigin::signed(vault_id.account_id),
			vault_id.currencies,
			backing_collateral - minimum_backing_collateral - 1
		));
	});
}

#[test]
fn is_collateral_below_threshold_true_succeeds() {
	run_test(|| {
		let collateral = DEFAULT_COLLATERAL;
		let wrapped_amount = DEFAULT_COLLATERAL / 2;
		let threshold = FixedU128::checked_from_rational(201, 100).unwrap(); // 201%

		convert_to.mock_safe(move |_, _| MockResult::Return(Ok(wrapped(collateral))));

		assert_eq!(
			VaultRegistry::is_collateral_below_threshold(
				&amount(collateral),
				&wrapped(wrapped_amount),
				threshold
			),
			Ok(true)
		);
	})
}

#[test]
fn is_collateral_below_threshold_false_succeeds() {
	run_test(|| {
		let collateral = DEFAULT_COLLATERAL;
		let wrapped_amount = 50;
		let threshold = FixedU128::checked_from_rational(200, 100).unwrap(); // 200%

		convert_to.mock_safe(move |_, _| MockResult::Return(Ok(wrapped(collateral))));

		assert_eq!(
			VaultRegistry::is_collateral_below_threshold(
				&amount(collateral),
				&wrapped(wrapped_amount),
				threshold
			),
			Ok(false)
		);
	})
}

#[test]
fn calculate_max_wrapped_from_collateral_for_threshold_succeeds() {
	run_test(|| {
		let collateral: u128 = u64::MAX as u128;
		let threshold = FixedU128::checked_from_rational(200, 100).unwrap(); // 200%

		convert_to.mock_safe(move |_, _| MockResult::Return(Ok(wrapped(collateral))));

		assert_eq!(
			VaultRegistry::calculate_max_wrapped_from_collateral_for_threshold(
				&amount(collateral),
				DEFAULT_WRAPPED_CURRENCY,
				threshold
			),
			Ok(wrapped((u64::MAX / 2) as u128))
		);
	})
}

#[test]
#[cfg_attr(feature = "skip-slow-tests", ignore)]
fn test_threshold_equivalent_to_legacy_calculation() {
	/// old version
	fn legacy_calculate_max_wrapped_from_collateral_for_threshold(
		collateral: BalanceOf<Test>,
		threshold: u128,
	) -> Result<BalanceOf<Test>, DispatchError> {
		let granularity = 5;
		// convert the collateral to wrapped
		let collateral_in_wrapped = convert_to(DEFAULT_WRAPPED_CURRENCY, amount(collateral))?;
		let collateral_in_wrapped = U256::from(collateral_in_wrapped.amount());

		// calculate how many tokens should be maximally issued given the threshold
		let scaled_collateral_in_wrapped = collateral_in_wrapped
			.checked_mul(U256::from(10).pow(granularity.into()))
			.ok_or(ArithmeticError::Overflow)?;
		let scaled_max_tokens =
			scaled_collateral_in_wrapped.checked_div(threshold.into()).unwrap_or(0.into());

		Ok(scaled_max_tokens.try_into()?)
	}

	run_test(|| {
		let threshold = FixedU128::checked_from_rational(199999, 100000).unwrap(); // 199.999%
		let random_start = 987529462328_u128;
		for wrapped_amount in random_start..random_start + 199999 {
			let old =
				legacy_calculate_max_wrapped_from_collateral_for_threshold(wrapped_amount, 199999)
					.unwrap();

			let new = VaultRegistry::calculate_max_wrapped_from_collateral_for_threshold(
				&amount(wrapped_amount),
				DEFAULT_WRAPPED_CURRENCY,
				threshold,
			)
			.unwrap();
			assert_eq!(wrapped(old), new);
		}
	})
}

#[test]
#[cfg_attr(feature = "skip-slow-tests", ignore)]
fn test_get_required_collateral_threshold_equivalent_to_legacy_calculation_() {
	// old version
	fn legacy_get_required_collateral_for_wrapped_with_threshold(
		xlm: BalanceOf<Test>,
		threshold: u128,
	) -> Result<BalanceOf<Test>, DispatchError> {
		let granularity = 5;
		let xlm = U256::from(xlm);

		// Step 1: inverse of the scaling applied in
		// calculate_max_wrapped_from_collateral_for_threshold

		// inverse of the div
		let xlm = xlm.checked_mul(threshold.into()).ok_or(ArithmeticError::Overflow)?;

		// To do the inverse of the multiplication, we need to do division, but
		// we need to round up. To round up (a/b), we need to do ((a+b-1)/b):
		let rounding_addition = U256::from(10).pow(granularity.into()) - U256::from(1);
		let xlm = (xlm + rounding_addition)
			.checked_div(U256::from(10).pow(granularity.into()))
			.ok_or(ArithmeticError::Underflow)?;

		// Step 2: convert the amount to collateral
		let amount_in_collateral =
			convert_to(DEFAULT_COLLATERAL_CURRENCY, wrapped(xlm.try_into()?))?;
		Ok(amount_in_collateral.amount())
	}

	run_test(|| {
		let threshold = FixedU128::checked_from_rational(199999, 100000).unwrap(); // 199.999%
		let random_start = 987529462328_u128;
		for xlm in random_start..random_start + 199999 {
			let old = legacy_get_required_collateral_for_wrapped_with_threshold(xlm, 199999);
			let new = VaultRegistry::get_required_collateral_for_wrapped_with_threshold(
				&wrapped(xlm),
				threshold,
				DEFAULT_COLLATERAL_CURRENCY,
			);
			assert_eq!(old.map(amount), new);
		}
	})
}

#[test]
fn get_required_collateral_for_wrapped_with_threshold_succeeds() {
	run_test(|| {
		let threshold = FixedU128::checked_from_rational(19999, 10000).unwrap(); // 199.99%
		let random_start = 987529387592_u128;
		for wrapped_amount in random_start..random_start + 19999 {
			let min_collateral = VaultRegistry::get_required_collateral_for_wrapped_with_threshold(
				&wrapped(wrapped_amount),
				threshold,
				DEFAULT_COLLATERAL_CURRENCY,
			)
			.unwrap();

			let max_wrapped_for_min_collateral =
				VaultRegistry::calculate_max_wrapped_from_collateral_for_threshold(
					&min_collateral,
					DEFAULT_WRAPPED_CURRENCY,
					threshold,
				)
				.unwrap();

			let max_wrapped_for_below_min_collateral =
				VaultRegistry::calculate_max_wrapped_from_collateral_for_threshold(
					&amount(min_collateral.amount() - 1),
					DEFAULT_WRAPPED_CURRENCY,
					threshold,
				)
				.unwrap();

			// Check that the amount we found is indeed the lowest amount that is sufficient for
			// `wrapped`
			// We have to be a little generous here, because the conversion from collateral to
			// wrapped may be rounded. So we give 10^(wrapped_decimals - collateral_decimals) as
			// tolerance.
			let tolerance = 10u128.pow(
				<Test as oracle::Config>::DecimalsLookup::decimals(DEFAULT_WRAPPED_CURRENCY)
					.saturating_sub(<Test as oracle::Config>::DecimalsLookup::decimals(
						DEFAULT_COLLATERAL_CURRENCY,
					)),
			);
			assert!(max_wrapped_for_min_collateral.amount() + tolerance >= wrapped_amount);
			assert!(max_wrapped_for_below_min_collateral.amount() - tolerance < wrapped_amount);
		}
	})
}

mod custom_secure_threshold_tests {
	use sp_runtime::traits::CheckedMul;

	use super::{assert_eq, *};

	#[test]
	fn set_custom_secure_threshold_succeeds() {
		run_test(|| {
			let id = vault_id(4);
			let collateral = 50;
			create_vault_with_collateral(&id, collateral);

			let system_threshold: UnsignedFixedPoint =
				VaultRegistry::secure_collateral_threshold(&id.currencies)
					.expect("Unable to get secure collateral threshold");
			let default_threshold = VaultRegistry::get_vault_secure_threshold(&id)
				.expect("Unable to get default secure threshold for sample vault");
			assert_eq!(default_threshold, system_threshold);

			// set a custom threshold
			let double_system_threshold = system_threshold
				.checked_mul(&UnsignedFixedPoint::checked_from_integer(2u32).unwrap())
				.unwrap();
			assert_ok!(VaultRegistry::try_set_vault_custom_secure_threshold(
				&id,
				Some(double_system_threshold)
			)); // double the threshold
			let new_threshold = VaultRegistry::get_vault_secure_threshold(&id)
				.expect("Unable to get custom secure threshold for sample vault");
			assert_eq!(new_threshold, double_system_threshold);

			// reset threshold
			assert_ok!(VaultRegistry::try_set_vault_custom_secure_threshold(&id, None));
			let reset_threshold = VaultRegistry::get_vault_secure_threshold(&id)
				.expect("Unable to get reset secure threshold for sample vault");
			assert_eq!(reset_threshold, system_threshold);
		})
	}

	#[test]
	fn set_custom_secure_threshold_sets_issuable_tokens() {
		run_test(|| {
			let id = vault_id(4);
			let collateral = 50;
			create_vault_with_collateral(&id, collateral);
			let two = UnsignedFixedPoint::checked_from_integer(2u32).unwrap();
			let system_secure_threshold: UnsignedFixedPoint =
				VaultRegistry::secure_collateral_threshold(&id.currencies)
					.expect("Unable to get secure collateral threshold");
			let issuable_tokens_before = VaultRegistry::get_issuable_tokens_from_vault(&id)
				.expect("Sample vault is unable to issue tokens");

			assert_ok!(VaultRegistry::try_set_vault_custom_secure_threshold(
				&id,
				Some(system_secure_threshold.checked_mul(&two).unwrap())
			)); // double the threshold
			let issuable_tokens_after = VaultRegistry::get_issuable_tokens_from_vault(&id)
				.expect("Sample vault is unable to issue tokens");

			assert_eq!(issuable_tokens_before.checked_div(&two).unwrap(), issuable_tokens_after);
		})
	}
}

mod liquidation_threshold_tests {
	use crate::mock::{AccountId, Balance, BlockNumber};

	use super::{assert_eq, *};

	fn setup() -> Vault<AccountId, BlockNumber, Balance, CurrencyId, UnsignedFixedPoint> {
		let id = create_sample_vault();

		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(5_000)),);
		let res = VaultRegistry::issue_tokens(&id, &wrapped(5_000));
		assert_ok!(res);

		let mut vault = VaultRegistry::get_vault_from_id(&id).unwrap();
		vault.issued_tokens = 5_000;
		vault.to_be_issued_tokens = 4_000;
		vault.to_be_redeemed_tokens = 2_000;

		vault
	}

	#[test]
	fn is_vault_below_liquidation_threshold_false_succeeds() {
		run_test(|| {
			let vault = setup();
			let backing_collateral = vault.issued_tokens * 2;
			VaultRegistry::get_backing_collateral
				.mock_safe(move |_| MockResult::Return(Ok(amount(backing_collateral))));
			assert_eq!(
				VaultRegistry::is_vault_below_liquidation_threshold(&vault, FixedU128::from(2)),
				Ok(false)
			);
		})
	}

	#[test]
	fn is_vault_below_liquidation_threshold_true_succeeds() {
		run_test(|| {
			let vault = setup();
			let issued_tokens_as_collateral =
				Oracle::convert(&wrapped(vault.issued_tokens), vault.id.currencies.collateral)
					.expect("Conversion should work")
					.amount();
			let backing_collateral = issued_tokens_as_collateral * 2 - 1;
			VaultRegistry::get_backing_collateral
				.mock_safe(move |_| MockResult::Return(Ok(amount(backing_collateral))));
			assert_eq!(
				VaultRegistry::is_vault_below_liquidation_threshold(&vault, FixedU128::from(2)),
				Ok(true)
			);
		})
	}
}

#[test]
fn get_collateralization_from_vault_fails_with_no_tokens_issued() {
	run_test(|| {
		// vault has no tokens issued yet
		let id = create_sample_vault();

		assert_err!(
			VaultRegistry::get_collateralization_from_vault(id, false),
			TestError::NoTokensIssued
		);
	})
}

#[test]
fn get_collateralization_from_vault_succeeds() {
	run_test(|| {
		let issue_tokens: u128 = DEFAULT_COLLATERAL / 10 / 2; // = 5
		let id = create_sample_vault_and_issue_tokens(issue_tokens);

		assert_eq!(
			VaultRegistry::get_collateralization_from_vault(id, false),
			Ok(FixedU128::checked_from_rational(200, 100).unwrap())
		);
	})
}

#[test]
fn get_unsettled_collateralization_from_vault_succeeds() {
	run_test(|| {
		let issue_tokens: u128 = DEFAULT_COLLATERAL / 10 / 5; // = 2
		let id = create_sample_vault_and_issue_tokens(issue_tokens);

		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(issue_tokens)),);

		assert_eq!(
			VaultRegistry::get_collateralization_from_vault(id, true),
			Ok(FixedU128::checked_from_rational(500, 100).unwrap())
		);
	})
}

#[test]
fn get_settled_collateralization_from_vault_succeeds() {
	run_test(|| {
		// currency_to_usd is / 10 and we issue 2 * amount
		let issue_tokens: u128 = 100000 / 10 / 5; // 2000
		let id = create_sample_vault_and_issue_tokens(issue_tokens);

		assert_ok!(VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(issue_tokens)));

		assert_eq!(
			VaultRegistry::get_collateralization_from_vault(id, false),
			Ok(FixedU128::checked_from_rational(250, 100).unwrap())
		);
	})
}

mod get_vaults_below_premium_collateralization_tests {
	use super::{assert_eq, *};

	/// sets premium_redeem threshold to 1
	pub fn run_test(test: impl FnOnce()) {
		super::run_test(|| {
			VaultRegistry::_set_secure_collateral_threshold(
				DEFAULT_CURRENCY_PAIR,
				FixedU128::from_float(0.001),
			);
			VaultRegistry::_set_premium_redeem_threshold(DEFAULT_CURRENCY_PAIR, FixedU128::one());

			test()
		})
	}

	fn add_vault(id: DefaultVaultId<Test>, issued_tokens: u128, collateral: u128) {
		create_vault_with_collateral(&id, collateral);

		VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(issued_tokens)).unwrap();
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(issued_tokens)));

		// sanity check
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_eq!(vault.data.issued_tokens, issued_tokens);
		assert_eq!(vault.data.to_be_redeemed_tokens, 0);
	}

	#[test]
	fn get_vaults_below_premium_collateralization_fails() {
		run_test(|| {
			add_vault(vault_id(4), 50, 100);

			assert_err!(
				VaultRegistry::get_premium_redeem_vaults(),
				TestError::NoVaultUnderThePremiumRedeemThreshold
			);
		})
	}

	#[test]
	fn get_vaults_below_premium_collateralization_succeeds() {
		run_test(|| {
			let id1 = vault_id(3);
			let issue_tokens1 = 5_000;
			let collateral1 = Oracle::convert(&wrapped(issue_tokens1), id1.collateral_currency())
				.expect("Conversion should work")
				.amount();
			let collateral1 = collateral1 - 1;

			let id2 = vault_id(4);
			let issue_tokens2: u128 = 6_000;
			let collateral2 = Oracle::convert(&wrapped(issue_tokens2), id2.collateral_currency())
				.expect("Conversion should work")
				.amount();
			let collateral2 = collateral2 - 12;

			add_vault(id1.clone(), issue_tokens1, collateral1);
			add_vault(id2.clone(), issue_tokens2, collateral2);

			assert_eq!(
				VaultRegistry::get_premium_redeem_vaults(),
				Ok(vec![(id2, wrapped(issue_tokens2)), (id1, wrapped(issue_tokens1))])
			);
		})
	}

	#[test]
	fn get_vaults_below_premium_collateralization_filters_banned_and_sufficiently_collateralized_vaults(
	) {
		run_test(|| {
			// not returned, because it is not under premium threshold (which is set to 100% for
			// this test)
			let id1 = vault_id(3);
			let issue_tokens1 = 5_000;
			let collateral1 = Oracle::convert(&wrapped(issue_tokens1), id1.collateral_currency())
				.expect("Conversion should work")
				.amount();
			add_vault(id1, issue_tokens1, collateral1);

			// returned
			let id2 = vault_id(4);
			let issue_tokens2: u128 = 5_000;
			let collateral2 = Oracle::convert(&wrapped(issue_tokens2), id2.collateral_currency())
				.expect("Conversion should work")
				.amount();
			let collateral2 = collateral2 - 1;
			add_vault(id2.clone(), issue_tokens2, collateral2);

			// not returned because it's banned
			let id3 = vault_id(5);
			let issue_tokens3: u128 = 5_000;
			let collateral3 = Oracle::convert(&wrapped(issue_tokens3), id3.collateral_currency())
				.expect("Conversion should work")
				.amount();
			let collateral3 = collateral3 - 1;
			add_vault(id3.clone(), issue_tokens3, collateral3);
			let mut vault3 = VaultRegistry::get_active_rich_vault_from_id(&id3).unwrap();
			vault3.ban_until(1000);

			assert_eq!(
				VaultRegistry::get_premium_redeem_vaults(),
				Ok(vec!((id2, wrapped(issue_tokens2))))
			);
		})
	}
}

mod get_vaults_with_issuable_tokens_tests {
	use super::{assert_eq, *};

	#[test]
	fn get_vaults_with_issuable_tokens_succeeds() {
		run_test(|| {
			let id1 = vault_id(3);
			let collateral1 = 100;
			create_vault_with_collateral(&id1, collateral1);
			let issuable_tokens1 = VaultRegistry::get_issuable_tokens_from_vault(&id1)
				.expect("Sample vault is unable to issue tokens");

			let id2 = vault_id(4);
			let collateral2 = 50;
			create_vault_with_collateral(&id2, collateral2);
			let issuable_tokens2 = VaultRegistry::get_issuable_tokens_from_vault(&id2)
				.expect("Sample vault is unable to issue tokens");

			// Check result is ordered in descending order
			assert_eq!(issuable_tokens1.gt(&issuable_tokens2).unwrap(), true);
			assert_eq!(
				VaultRegistry::get_vaults_with_issuable_tokens(),
				Ok(vec!((id1, issuable_tokens1), (id2, issuable_tokens2)))
			);
		})
	}

	#[test]
	fn get_vaults_with_issuable_tokens_succeeds_with_custom_vault_secure_thresholds() {
		run_test(|| {
			let id1 = vault_id(3);
			let collateral1 = 100;
			create_vault_with_collateral(&id1, collateral1);
			let issuable_tokens1 = VaultRegistry::get_issuable_tokens_from_vault(&id1)
				.expect("Sample vault is unable to issue tokens");

			let id2 = vault_id(4);
			let collateral2 = 200;
			create_vault_with_collateral(&id2, collateral2);
			assert_ok!(VaultRegistry::try_set_vault_custom_secure_threshold(&id2, Some(5.into()))); // 500% custom threshold
			let issuable_tokens2 = VaultRegistry::get_issuable_tokens_from_vault(&id2)
				.expect("Sample vault is unable to issue tokens");

			// Check result is ordered in descending order
			assert_eq!(issuable_tokens1.gt(&issuable_tokens2).unwrap(), true);
			assert_eq!(
				VaultRegistry::get_vaults_with_issuable_tokens(),
				Ok(vec!((id1, issuable_tokens1), (id2, issuable_tokens2)))
			);
		})
	}

	#[test]
	fn get_vaults_with_issuable_tokens_succeeds_when_there_are_liquidated_vaults() {
		run_test(|| {
			let id1 = vault_id(3);
			let collateral1 = 100;
			create_vault_with_collateral(&id1, collateral1);
			let issuable_tokens1 = VaultRegistry::get_issuable_tokens_from_vault(&id1)
				.expect("Sample vault is unable to issue tokens");

			let id2 = vault_id(4);
			let collateral2 = 50;
			create_vault_with_collateral(&id2, collateral2);

			// liquidate vault
			assert_ok!(VaultRegistry::liquidate_vault(&id2));

			assert_eq!(
				VaultRegistry::get_vaults_with_issuable_tokens(),
				Ok(vec!((id1, issuable_tokens1)))
			);
		})
	}

	#[test]
	fn get_vaults_with_issuable_tokens_filters_out_banned_vaults() {
		run_test(|| {
			let id1 = vault_id(3);
			let collateral1 = 100;
			create_vault_with_collateral(&id1, collateral1);
			let issuable_tokens1 = VaultRegistry::get_issuable_tokens_from_vault(&id1)
				.expect("Sample vault is unable to issue tokens");

			let id2 = vault_id(4);
			let collateral2 = 50;
			create_vault_with_collateral(&id2, collateral2);

			// ban the vault
			let mut vault = VaultRegistry::get_rich_vault_from_id(&id2).unwrap();
			vault.ban_until(1000);

			let issuable_tokens2 = VaultRegistry::get_issuable_tokens_from_vault(&id2)
				.expect("Sample vault is unable to issue tokens");

			assert!(issuable_tokens2.is_zero());

			// Check that the banned vault is not returned by get_vaults_with_issuable_tokens
			assert_eq!(
				VaultRegistry::get_vaults_with_issuable_tokens(),
				Ok(vec!((id1, issuable_tokens1)))
			);
		})
	}

	#[test]
	fn get_vaults_with_issuable_tokens_filters_out_vault_that_do_not_accept_new_issues() {
		run_test(|| {
			let id1 = vault_id(3);
			let collateral1 = 100;
			create_vault_with_collateral(&id1, collateral1);
			let issuable_tokens1 = VaultRegistry::get_issuable_tokens_from_vault(&id1)
				.expect("Sample vault is unable to issue tokens");

			let id2 = vault_id(4);
			let collateral2 = 50;
			create_vault_with_collateral(&id2, collateral2);
			assert_ok!(VaultRegistry::accept_new_issues(
				RuntimeOrigin::signed(id2.account_id),
				id2.currencies,
				false
			));

			// Check that the vault that does not accept issues is not returned by
			// get_vaults_with_issuable_tokens
			assert_eq!(
				VaultRegistry::get_vaults_with_issuable_tokens(),
				Ok(vec!((id1, issuable_tokens1)))
			);
		})
	}

	#[test]
	fn get_vaults_with_issuable_tokens_returns_empty() {
		run_test(|| {
			let issue_tokens: u128 = DEFAULT_COLLATERAL / 2;
			let id = create_sample_vault();

			VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(issue_tokens)).unwrap();
			// issue DEFAULT_COLLATERAL / 2 tokens at 200% rate
			assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(issue_tokens)));
			let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
			assert_eq!(vault.data.issued_tokens, issue_tokens);
			assert_eq!(vault.data.to_be_redeemed_tokens, 0);

			// update the exchange rate
			convert_to.mock_safe(convert_with_exchange_rate(2));
			VaultRegistry::_set_secure_collateral_threshold(
				DEFAULT_CURRENCY_PAIR,
				FixedU128::checked_from_rational(150, 100).unwrap(), // 150%
			);

			assert_eq!(VaultRegistry::get_vaults_with_issuable_tokens(), Ok(vec!()));
		})
	}
}

mod get_vaults_with_redeemable_tokens_test {
	use super::{assert_eq, *};

	fn create_vault_with_issue(id: DefaultVaultId<Test>, to_issue: u128) {
		create_vault(id.clone());
		VaultRegistry::try_increase_to_be_issued_tokens(&id, &wrapped(to_issue)).unwrap();
		assert_ok!(VaultRegistry::issue_tokens(&id, &wrapped(to_issue)));
		let vault = VaultRegistry::get_active_rich_vault_from_id(&id).unwrap();
		assert_eq!(vault.data.issued_tokens, to_issue);
		assert_eq!(vault.data.to_be_redeemed_tokens, 0);
	}

	#[test]
	fn get_vaults_with_redeemable_tokens_returns_empty() {
		run_test(|| {
			// create a vault with no redeemable tokens
			create_sample_vault();
			// nothing issued, so nothing can be redeemed
			assert_eq!(VaultRegistry::get_vaults_with_redeemable_tokens(), Ok(vec!()));
		})
	}

	#[test]
	fn get_vaults_with_redeemable_tokens_succeeds() {
		run_test(|| {
			let id1 = vault_id(3);
			let issued_tokens1: u128 = 10;
			create_vault_with_issue(id1.clone(), issued_tokens1);

			let id2 = vault_id(4);
			let issued_tokens2: u128 = 20;
			create_vault_with_issue(id2.clone(), issued_tokens2);

			// Check result is ordered in descending order
			assert_eq!(issued_tokens2.gt(&issued_tokens1), true);
			assert_eq!(
				VaultRegistry::get_vaults_with_redeemable_tokens(),
				Ok(vec!((id2, wrapped(issued_tokens2)), (id1, wrapped(issued_tokens1))))
			);
		})
	}

	#[test]
	fn get_vaults_with_redeemable_tokens_filters_out_banned_vaults() {
		run_test(|| {
			let id1 = vault_id(3);
			let issued_tokens1: u128 = 10;
			create_vault_with_issue(id1.clone(), issued_tokens1);

			let id2 = vault_id(4);
			let issued_tokens2: u128 = 20;
			create_vault_with_issue(id2.clone(), issued_tokens2);

			// ban the vault
			let mut vault = VaultRegistry::get_rich_vault_from_id(&id2).unwrap();
			vault.ban_until(1000);

			// Check that the banned vault is not returned by get_vaults_with_redeemable_tokens
			assert_eq!(
				VaultRegistry::get_vaults_with_redeemable_tokens(),
				Ok(vec!((id1, wrapped(issued_tokens1))))
			);
		})
	}

	#[test]
	fn get_vaults_with_issuable_tokens_filters_out_liquidated_vaults() {
		run_test(|| {
			let id1 = vault_id(3);
			let issued_tokens1: u128 = 10;
			create_vault_with_issue(id1.clone(), issued_tokens1);

			let id2 = vault_id(4);
			let issued_tokens2: u128 = 20;
			create_vault_with_issue(id2.clone(), issued_tokens2);

			// liquidate vault
			assert_ok!(VaultRegistry::liquidate_vault(&id2));

			assert_eq!(
				VaultRegistry::get_vaults_with_redeemable_tokens(),
				Ok(vec!((id1, wrapped(issued_tokens1))))
			);
		})
	}
}

#[test]
fn test_try_increase_to_be_replaced_tokens() {
	run_test(|| {
		let issue_tokens: u128 = 4;
		let vault_id = create_sample_vault_and_issue_tokens(issue_tokens);
		assert_ok!(VaultRegistry::try_increase_to_be_redeemed_tokens(&vault_id, &wrapped(1)));

		let total_wrapped =
			VaultRegistry::try_increase_to_be_replaced_tokens(&vault_id, &wrapped(2)).unwrap();
		assert!(total_wrapped == wrapped(2));

		// check that we can't request more than we have issued tokens
		assert_noop!(
			VaultRegistry::try_increase_to_be_replaced_tokens(&vault_id, &wrapped(3)),
			TestError::InsufficientTokensCommitted
		);

		// check that we can't request replacement for tokens that are marked as to-be-redeemed
		assert_noop!(
			VaultRegistry::try_increase_to_be_replaced_tokens(&vault_id, &wrapped(2)),
			TestError::InsufficientTokensCommitted
		);

		let mut vault = VaultRegistry::get_active_rich_vault_from_id(&vault_id).unwrap();
		vault.increase_available_replace_collateral(&griefing(10)).unwrap();

		let total_wrapped =
			VaultRegistry::try_increase_to_be_replaced_tokens(&vault_id, &wrapped(1)).unwrap();
		assert_eq!(total_wrapped, wrapped(3));

		// check that to_be_replaced_tokens is was written to storage
		let vault = VaultRegistry::get_active_vault_from_id(&vault_id).unwrap();
		assert_eq!(vault.to_be_replaced_tokens, 3);
	})
}

#[test]
fn test_decrease_to_be_replaced_tokens_over_capacity() {
	run_test(|| {
		let issue_tokens: u128 = 4;
		let vault_id = create_sample_vault_and_issue_tokens(issue_tokens);

		assert_ok!(VaultRegistry::try_increase_to_be_replaced_tokens(&vault_id, &wrapped(4),));
		let mut vault = VaultRegistry::get_active_rich_vault_from_id(&vault_id).unwrap();
		vault.increase_available_replace_collateral(&griefing(10)).unwrap();

		let (tokens, collateral) =
			VaultRegistry::decrease_to_be_replaced_tokens(&vault_id, &wrapped(5)).unwrap();
		assert_eq!(tokens, wrapped(4));
		assert_eq!(collateral, griefing(10));
	})
}

#[test]
fn test_decrease_to_be_replaced_tokens_below_capacity() {
	run_test(|| {
		let issue_tokens: u128 = 4;
		let vault_id = create_sample_vault_and_issue_tokens(issue_tokens);

		assert_ok!(VaultRegistry::try_increase_to_be_replaced_tokens(&vault_id, &wrapped(4),));
		let mut vault = VaultRegistry::get_active_rich_vault_from_id(&vault_id).unwrap();
		vault.increase_available_replace_collateral(&griefing(10)).unwrap();

		let (tokens, collateral) =
			VaultRegistry::decrease_to_be_replaced_tokens(&vault_id, &wrapped(3)).unwrap();
		assert_eq!(tokens, wrapped(3));
		assert_eq!(collateral, griefing(7));
	})
}

#[test]
fn test_offchain_worker_unsigned_transaction_submission() {
	let mut externalities = crate::mock::ExtBuilder::build();
	let (pool, pool_state) = TestTransactionPoolExt::new();
	externalities.register_extension(TransactionPoolExt::new(pool));

	externalities.execute_with(|| {
		// setup state:
		let id = vault_id(7);
		System::set_block_number(1);
		Security::<Test>::set_active_block_number(1);
		set_default_thresholds();
		VaultRegistry::insert_vault(&id, Vault::new(id.clone()));

		// mock that all vaults need to be liquidated
		VaultRegistry::is_vault_below_liquidation_threshold
			.mock_safe(move |_, _| MockResult::Return(Ok(true)));

		// call the actual function we want to test
		VaultRegistry::_offchain_worker();

		// check that a transaction has been added to liquidate the vault
		let tx = pool_state.write().transactions.pop().unwrap();
		assert!(pool_state.read().transactions.is_empty());
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		assert_eq!(tx.signature, None); // unsigned
		assert_eq!(
			tx.call,
			crate::mock::RuntimeCall::VaultRegistry(
				crate::Call::report_undercollateralized_vault { vault_id: id }
			)
		);
	})
}

mod integration {
	use super::{assert_eq, *};
	use oracle::OracleApi;

	fn to_usd(amount: &Balance, currency: &CurrencyId) -> Balance {
		<Test as reward_distribution::Config>::OracleApi::currency_to_usd(amount, currency)
			.expect("prices have been set in mock configuration")
	}
	#[test]
	fn integration_single_vault_receives_per_block_reward() {
		run_test(|| {
			//set up reward values
			let initial_block_number = 1u64;
			let reward_per_block: u64 = 1000u64;
			assert_ok!(ext::staking::add_reward_currency::<Test>(DEFAULT_WRAPPED_CURRENCY));
			assert_ok!(<reward_distribution::Pallet<Test>>::set_reward_per_block(
				RawOrigin::Root.into(),
				reward_per_block.into()
			));
			//register vault and issue tokens.
			//the number of issue tokens is not relevant for these tests
			let issue_tokens: u128 = 2;
			let id = create_vault_and_issue_tokens(
				issue_tokens,
				DEFAULT_COLLATERAL,
				COLLATERAL_1_VAULT_1,
			);
			assert_eq!(
				<Test as fee::Config>::VaultRewards::get_stake(&id.collateral_currency(), &id),
				Ok(DEFAULT_COLLATERAL)
			);
			assert_eq!(<pallet_balances::Pallet<Test>>::free_balance(&id.account_id), 0u128);

			//distribute fee rewards
			<reward_distribution::Pallet<Test>>::execute_on_init((initial_block_number + 1).into());
			//collect rewards
			let origin = RuntimeOrigin::signed(id.account_id);
			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				origin.into(),
				id.clone(),
				DEFAULT_NATIVE_CURRENCY,
				None,
			));
			assert_eq!(
				<pallet_balances::Pallet<Test>>::free_balance(&id.account_id),
				reward_per_block.into()
			);
		});
	}

	#[test]
	fn integration_multiple_vault_same_collateral_per_block_reward() {
		run_test(|| {
			//ARRANGE
			//set up reward values
			let initial_block_number = 1u64;
			let reward_per_block: u128 = 100000u128;
			assert_ok!(ext::staking::add_reward_currency::<Test>(DEFAULT_WRAPPED_CURRENCY));
			assert_ok!(<reward_distribution::Pallet<Test>>::set_reward_per_block(
				RawOrigin::Root.into(),
				reward_per_block.into()
			));

			//register vaults and issue tokens.
			let issue_tokens: u128 = 2;

			let collateral_vault_1 = 1000u128;
			let id_1 = create_vault_and_issue_tokens(
				issue_tokens,
				collateral_vault_1,
				COLLATERAL_1_VAULT_1,
			);
			assert_eq!(
				<Test as fee::Config>::VaultRewards::get_stake(&id_1.collateral_currency(), &id_1),
				Ok(collateral_vault_1)
			);

			let collateral_vault_2 = 5000u128;
			let id_2 = create_vault_and_issue_tokens(
				issue_tokens,
				collateral_vault_2,
				COLLATERAL_1_VAULT_2,
			);
			assert_eq!(
				<Test as fee::Config>::VaultRewards::get_stake(&id_2.collateral_currency(), &id_2),
				Ok(collateral_vault_2)
			);

			//ACT - distribute fee rewards
			<reward_distribution::Pallet<Test>>::execute_on_init((initial_block_number + 1).into());

			//collect rewards for vault 1 and 2
			let origin_1 = RuntimeOrigin::signed(id_1.account_id);
			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				origin_1.into(),
				id_1.clone(),
				DEFAULT_NATIVE_CURRENCY,
				None,
			));

			let origin_2 = RuntimeOrigin::signed(id_2.account_id);
			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				origin_2.into(),
				id_2.clone(),
				DEFAULT_NATIVE_CURRENCY,
				None,
			));

			// ASSERT
			//MODEL: reward_vault_i = ( usdPrice(vault_i_collateral)/ ( SUM_i {
			// usdPrice(vault_i_collateral) })*reward_per_block
			let vault_1_collateral_usd = to_usd(&collateral_vault_1, &DEFAULT_COLLATERAL_CURRENCY);
			let vault_2_collateral_usd = to_usd(&collateral_vault_2, &DEFAULT_COLLATERAL_CURRENCY);

			let expected_value_vault_1: u128 = ((vault_1_collateral_usd as f64 /
				(vault_1_collateral_usd + vault_2_collateral_usd) as f64) *
				reward_per_block as f64)
				.floor() as u128;

			//collect rewards for vault 2
			let expected_value_vault_2: u128 = ((vault_2_collateral_usd as f64 /
				(vault_1_collateral_usd + vault_2_collateral_usd) as f64) *
				reward_per_block as f64)
				.floor() as u128;

			assert_eq!(
				<pallet_balances::Pallet<Test>>::free_balance(&id_1.account_id),
				expected_value_vault_1.into()
			);
			assert_eq!(
				<pallet_balances::Pallet<Test>>::free_balance(&id_2.account_id),
				expected_value_vault_2.into()
			);
		});
	}

	#[test]
	fn integration_multiple_vault_multiple_collateral_per_block_reward() {
		run_test(|| {
			//ARRANGE
			let init_block = 1u32;
			//set up reward values and threshold
			assert_ok!(ext::staking::add_reward_currency::<Test>(DEFAULT_WRAPPED_CURRENCY));
			let reward_per_block: u128 = 100000;
			assert_ok!(<reward_distribution::Pallet<Test>>::set_reward_per_block(
				RawOrigin::Root.into(),
				reward_per_block.into()
			));

			//register vaults and issue tokens.
			let issue_tokens: u128 = 2;

			let collateral_vault_1 = 1000u128;
			let id_1 = create_vault_and_issue_tokens(
				issue_tokens,
				collateral_vault_1,
				COLLATERAL_1_VAULT_1,
			);
			assert_eq!(
				<Test as fee::Config>::VaultRewards::get_stake(&id_1.collateral_currency(), &id_1),
				Ok(collateral_vault_1)
			);

			let collateral_vault_2 = 5000u128;
			let id_2 = create_vault_and_issue_tokens(
				issue_tokens,
				collateral_vault_2,
				COLLATERAL_1_VAULT_2,
			);
			assert_eq!(
				<Test as fee::Config>::VaultRewards::get_stake(&id_2.collateral_currency(), &id_2),
				Ok(collateral_vault_2)
			);

			let collateral_vault_3 = 3000u128;
			let id_3 = create_vault_and_issue_tokens(
				issue_tokens,
				collateral_vault_3,
				COLLATERAL_2_VAULT_1,
			);
			assert_eq!(
				<Test as fee::Config>::VaultRewards::get_stake(&id_3.collateral_currency(), &id_3),
				Ok(collateral_vault_3)
			);

			let collateral_vault_4 = 2000u128;
			let id_4 = create_vault_and_issue_tokens(
				issue_tokens,
				collateral_vault_4,
				COLLATERAL_2_VAULT_2,
			);
			assert_eq!(
				<Test as fee::Config>::VaultRewards::get_stake(&id_4.collateral_currency(), &id_4),
				Ok(collateral_vault_4)
			);

			let vault_1_collateral_usd =
				to_usd(&collateral_vault_1, &COLLATERAL_1_VAULT_1.collateral_currency());
			let vault_2_collateral_usd =
				to_usd(&collateral_vault_2, &COLLATERAL_1_VAULT_2.collateral_currency());
			let vault_3_collateral_usd =
				to_usd(&collateral_vault_3, &COLLATERAL_2_VAULT_1.collateral_currency());
			let vault_4_collateral_usd =
				to_usd(&collateral_vault_4, &COLLATERAL_2_VAULT_2.collateral_currency());

			let total_usd_amount = vault_1_collateral_usd +
				vault_2_collateral_usd +
				vault_3_collateral_usd +
				vault_4_collateral_usd;

			let expected_value_vault_1: u128 = ((((vault_1_collateral_usd + vault_2_collateral_usd)
				as f64 / total_usd_amount as f64) *
				reward_per_block as f64)
				.floor() * (collateral_vault_1 as f64 /
				(collateral_vault_1 + collateral_vault_2) as f64))
				.floor() as u128;

			let expected_value_vault_2: u128 = ((((vault_1_collateral_usd + vault_2_collateral_usd)
				as f64 / total_usd_amount as f64) *
				reward_per_block as f64)
				.floor() * (collateral_vault_2 as f64 /
				(collateral_vault_1 + collateral_vault_2) as f64))
				.floor() as u128;

			let expected_value_vault_3: u128 = ((((vault_4_collateral_usd + vault_3_collateral_usd)
				as f64 / total_usd_amount as f64) *
				reward_per_block as f64)
				.floor() * (collateral_vault_3 as f64 /
				(collateral_vault_3 + collateral_vault_4) as f64))
				.floor() as u128;

			let expected_value_vault_4: u128 = ((((vault_4_collateral_usd + vault_3_collateral_usd)
				as f64 / total_usd_amount as f64) *
				reward_per_block as f64)
				.floor() * (collateral_vault_4 as f64 /
				(collateral_vault_3 + collateral_vault_4) as f64))
				.floor() as u128;

			//ACT
			//distribute fee rewards
			<reward_distribution::Pallet<Test>>::execute_on_init((init_block + 1).into());
			assert_eq!(
				<reward_distribution::Pallet<Test>>::native_liability(),
				Some(reward_per_block)
			);

			//collect rewards
			let origin_1 = RuntimeOrigin::signed(id_1.account_id);
			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				origin_1.into(),
				id_1.clone(),
				DEFAULT_NATIVE_CURRENCY,
				None,
			));
			assert_eq!(
				<reward_distribution::Pallet<Test>>::native_liability(),
				Some(reward_per_block - expected_value_vault_1)
			);

			let origin_2 = RuntimeOrigin::signed(id_2.account_id);
			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				origin_2.into(),
				id_2.clone(),
				DEFAULT_NATIVE_CURRENCY,
				None,
			));

			let origin_3 = RuntimeOrigin::signed(id_3.account_id);
			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				origin_3.into(),
				id_3.clone(),
				DEFAULT_NATIVE_CURRENCY,
				None,
			));

			let origin_4 = RuntimeOrigin::signed(id_4.account_id);
			assert_ok!(<reward_distribution::Pallet<Test>>::collect_reward(
				origin_4.into(),
				id_4.clone(),
				DEFAULT_NATIVE_CURRENCY,
				None,
			));
			assert_eq!(<reward_distribution::Pallet<Test>>::native_liability(), Some(0));

			//ASSERT

			assert_eq!(
				<pallet_balances::Pallet<Test>>::free_balance(&id_1.account_id),
				expected_value_vault_1.into()
			);

			assert_eq!(
				<pallet_balances::Pallet<Test>>::free_balance(&id_2.account_id),
				expected_value_vault_2.into()
			);

			assert_eq!(
				<pallet_balances::Pallet<Test>>::free_balance(&id_3.account_id),
				expected_value_vault_3.into()
			);

			assert_eq!(
				<pallet_balances::Pallet<Test>>::free_balance(&id_4.account_id),
				expected_value_vault_4.into()
			);
		});
	}
}
