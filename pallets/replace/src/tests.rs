use frame_support::{assert_err, assert_ok};
use mocktopus::mocking::*;
use sp_core::H256;

use currency::Amount;
use stellar_relay::testing_utils::DEFAULT_STELLAR_PUBLIC_KEY;

use crate::{
	mock::{CurrencyId, *},
	*,
};

type Event = crate::Event<Test>;

macro_rules! assert_event_matches {
    ($( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {

        assert!(System::events().iter().any(|a| {
            match a.event {
                TestEvent::Replace( $( $pattern )|+ ) $( if $guard )? => true,
                _ => false
            }
        }));
    }
}
fn test_request() -> ReplaceRequest<AccountId, BlockNumber, Balance, CurrencyId> {
	ReplaceRequest {
		period: 0,
		new_vault: NEW_VAULT,
		old_vault: OLD_VAULT,
		accept_time: 1,
		amount: 10,
		asset: DEFAULT_WRAPPED_CURRENCY,
		griefing_collateral: 0,
		stellar_address: DEFAULT_STELLAR_PUBLIC_KEY,
		collateral: 20,
		status: ReplaceRequestStatus::Pending,
	}
}

fn griefing(amount: u128) -> Amount<Test> {
	Amount::new(amount, DEFAULT_NATIVE_CURRENCY)
}
fn wrapped(amount: u128) -> Amount<Test> {
	Amount::new(amount, DEFAULT_WRAPPED_CURRENCY)
}

mod request_replace_tests {
	use super::*;

	fn setup_mocks() {
		ext::vault_registry::ensure_not_banned::<Test>.mock_safe(|_| MockResult::Return(Ok(())));
		ext::vault_registry::requestable_to_be_replaced_tokens::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(wrapped(1000000))));
		ext::vault_registry::try_increase_to_be_replaced_tokens::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok(wrapped(2))));
		ext::fee::get_replace_griefing_collateral::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(griefing(20))));
		ext::vault_registry::transfer_funds::<Test>.mock_safe(|_, _, _| MockResult::Return(Ok(())));
	}

	#[test]
	fn test_request_replace_total_to_be_replace_above_dust_succeeds() {
		run_test(|| {
			setup_mocks();
			assert_ok!(Replace::_request_replace(OLD_VAULT, 1));
			assert_event_matches!(Event::RequestReplace { old_vault_id: OLD_VAULT, amount: 1, .. });
		})
	}

	#[test]
	fn test_request_replace_above_requestable_succeeds() {
		run_test(|| {
			setup_mocks();
			ext::vault_registry::requestable_to_be_replaced_tokens::<Test>
				.mock_safe(move |_| MockResult::Return(Ok(wrapped(5))));
			assert_ok!(Replace::_request_replace(OLD_VAULT, 10));
			assert_event_matches!(Event::RequestReplace { old_vault_id: OLD_VAULT, amount: 5, .. });
		})
	}

	#[test]
	fn test_request_replace_total_to_be_replace_below_dust_fails() {
		run_test(|| {
			setup_mocks();
			ext::vault_registry::try_increase_to_be_replaced_tokens::<Test>
				.mock_safe(|_, _| MockResult::Return(Ok(wrapped(1))));
			assert_err!(Replace::_request_replace(OLD_VAULT, 1), TestError::AmountBelowDustAmount);
		})
	}

	#[test]
	fn request_replace_should_fail_with_replace_amount_zero() {
		run_test(|| {
			setup_mocks();
			ext::vault_registry::try_increase_to_be_replaced_tokens::<Test>
				.mock_safe(|_, _| MockResult::Return(Ok(wrapped(1))));
			assert_err!(Replace::_request_replace(OLD_VAULT, 0), TestError::ReplaceAmountZero);
		})
	}
}

mod accept_replace_tests {
	use stellar_relay::testing_utils::RANDOM_STELLAR_PUBLIC_KEY;

	use super::*;

	fn setup_mocks() {
		ext::vault_registry::ensure_not_banned::<Test>.mock_safe(|_| MockResult::Return(Ok(())));
		ext::vault_registry::decrease_to_be_replaced_tokens::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok((wrapped(5), griefing(10)))));
		ext::vault_registry::try_deposit_collateral::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok(())));
		ext::vault_registry::try_increase_to_be_redeemed_tokens::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok(())));
		ext::vault_registry::try_increase_to_be_issued_tokens::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok(())));
		ext::vault_registry::transfer_funds::<Test>.mock_safe(|_, _, _| MockResult::Return(Ok(())));
	}

	#[test]
	fn test_accept_replace_succeeds() {
		run_test(|| {
			setup_mocks();
			let stellar_address = RANDOM_STELLAR_PUBLIC_KEY;
			assert_ok!(Replace::_accept_replace(OLD_VAULT, NEW_VAULT, 5, 10, stellar_address));
			assert_event_matches!(Event::AcceptReplace{
                replace_id: _,
                old_vault_id: OLD_VAULT,
                new_vault_id: NEW_VAULT,
                amount: 5,
				asset: DEFAULT_WRAPPED_CURRENCY,
                collateral: 10,
                stellar_address: addr} if addr == stellar_address);
		})
	}

	#[test]
	fn test_accept_replace_partial_accept_succeeds() {
		run_test(|| {
			// call to replace (5, 10), when there is only (4, 8) actually used
			setup_mocks();
			ext::vault_registry::decrease_to_be_replaced_tokens::<Test>
				.mock_safe(|_, _| MockResult::Return(Ok((wrapped(4), griefing(8)))));

			let stellar_address = RANDOM_STELLAR_PUBLIC_KEY;

			assert_ok!(Replace::_accept_replace(OLD_VAULT, NEW_VAULT, 5, 10, stellar_address));
			assert_event_matches!(Event::AcceptReplace{
                replace_id: _, 
                old_vault_id: OLD_VAULT, 
                new_vault_id: NEW_VAULT, 
                amount: 4, 
				asset: DEFAULT_WRAPPED_CURRENCY,
                collateral: 8,
                stellar_address: addr} if addr == stellar_address);
		})
	}

	#[test]
	fn test_accept_replace_below_dust_fails() {
		run_test(|| {
			setup_mocks();
			ext::vault_registry::decrease_to_be_replaced_tokens::<Test>
				.mock_safe(|_, _| MockResult::Return(Ok((wrapped(1), griefing(10)))));
			assert_err!(
				Replace::_accept_replace(OLD_VAULT, NEW_VAULT, 5, 10, RANDOM_STELLAR_PUBLIC_KEY),
				TestError::AmountBelowDustAmount
			);
		})
	}
}

mod execute_replace_test {
	use currency::Amount;
	use substrate_stellar_sdk::{types::AlphaNum4, Asset, Operation, PublicKey, StroopAmount};

	use super::*;

	fn setup_mocks() {
		ReplaceRequests::<Test>::insert(H256::zero(), {
			let mut replace = test_request();
			replace.old_vault = OLD_VAULT;
			replace.new_vault = NEW_VAULT;
			replace
		});

		Replace::replace_period.mock_safe(|| MockResult::Return(20));
		ext::stellar_relay::ensure_transaction_memo_matches_hash::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(())));
		ext::stellar_relay::validate_stellar_transaction::<Test>
			.mock_safe(move |_, _, _| MockResult::Return(Ok(())));
		ext::vault_registry::replace_tokens::<Test>
			.mock_safe(|_, _, _, _| MockResult::Return(Ok(())));
		Amount::<Test>::unlock_on.mock_safe(|_, _| MockResult::Return(Ok(())));
		ext::vault_registry::transfer_funds::<Test>.mock_safe(|_, _, _| MockResult::Return(Ok(())));

		ext::vault_registry::try_increase_to_be_redeemed_tokens::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok(())));
		ext::vault_registry::try_increase_to_be_issued_tokens::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok(())));
	}

	#[test]
	fn test_execute_replace_succeeds() {
		run_test(|| {
			setup_mocks();

			let op_amount = 10;
			let op = get_operation(op_amount, DEFAULT_STELLAR_PUBLIC_KEY);
			let structs_encoded =
				stellar_relay::testing_utils::create_dummy_scp_structs_with_operation_encoded(op);

			assert_ok!(Replace::_execute_replace(
				H256::zero(),
				structs_encoded.0,
				structs_encoded.1,
				structs_encoded.2
			));
			assert_event_matches!(Event::ExecuteReplace {
				replace_id: _,
				old_vault_id: OLD_VAULT,
				new_vault_id: NEW_VAULT
			});
		})
	}

	fn get_operation(amount: i64, stellar_address: [u8; 32]) -> Operation {
		let alpha_num4 = AlphaNum4 {
			asset_code: *b"USDC",
			issuer: PublicKey::PublicKeyTypeEd25519([
				20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231,
				46, 199, 108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
			]),
		};
		let stellar_asset = Asset::AssetTypeCreditAlphanum4(alpha_num4);
		let amount = StroopAmount(amount);
		let address = PublicKey::PublicKeyTypeEd25519(stellar_address);
		let op = Operation::new_payment(address, stellar_asset, amount)
			.expect("Should create operation");
		op
	}

	#[test]
	fn should_execute_cancelled_request() {
		run_test(|| {
			setup_mocks();

			ReplaceRequests::<Test>::insert(H256::zero(), {
				let mut replace = test_request();
				replace.old_vault = OLD_VAULT;
				replace.new_vault = NEW_VAULT;
				replace.status = ReplaceRequestStatus::Cancelled;
				replace
			});

			let op_amount = 10;
			let op = get_operation(op_amount, DEFAULT_STELLAR_PUBLIC_KEY);
			let structs_encoded =
				stellar_relay::testing_utils::create_dummy_scp_structs_with_operation_encoded(op);

			assert_ok!(Replace::_execute_replace(
				H256::zero(),
				structs_encoded.0,
				structs_encoded.1,
				structs_encoded.2
			));
			assert_event_matches!(Event::ExecuteReplace {
				replace_id: _,
				old_vault_id: OLD_VAULT,
				new_vault_id: NEW_VAULT
			});
		})
	}
}

mod cancel_replace_tests {
	use super::*;

	fn setup_mocks() {
		Replace::get_open_replace_request.mock_safe(move |_| {
			let mut replace = test_request();
			replace.old_vault = OLD_VAULT;
			replace.new_vault = NEW_VAULT;
			MockResult::Return(Ok(replace))
		});

		Replace::replace_period.mock_safe(|| MockResult::Return(20));
		ext::security::parachain_block_expired::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok(true)));
		ext::vault_registry::is_vault_liquidated::<Test>
			.mock_safe(|_| MockResult::Return(Ok(false)));
		ext::vault_registry::cancel_replace_tokens::<Test>
			.mock_safe(|_, _, _| MockResult::Return(Ok(())));
		ext::vault_registry::transfer_funds::<Test>.mock_safe(|_, _, _| MockResult::Return(Ok(())));
		ext::vault_registry::is_allowed_to_withdraw_collateral::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok(false)));
	}

	#[test]
	fn test_cancel_replace_succeeds() {
		run_test(|| {
			setup_mocks();
			assert_ok!(Replace::_cancel_replace(H256::zero(),));
			assert_event_matches!(Event::CancelReplace {
				replace_id: _,
				new_vault_id: NEW_VAULT,
				old_vault_id: OLD_VAULT,
				griefing_collateral: _
			});
		})
	}
}
