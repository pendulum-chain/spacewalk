use frame_support::{assert_err, assert_noop, assert_ok, dispatch::DispatchError};
use mocktopus::mocking::*;
use sp_core::H256;
use sp_runtime::traits::Zero;

use currency::{testing_constants::get_wrapped_currency_id, Amount};
use security::Pallet as Security;
use stellar_relay::testing_utils::RANDOM_STELLAR_PUBLIC_KEY;
use substrate_stellar_sdk::{types::AlphaNum4, Asset, Operation, PublicKey, StroopAmount};
use vault_registry::{DefaultVault, VaultStatus};

use crate::{
	ext,
	mock::*,
	types::{BalanceOf, RedeemRequest, RedeemRequestStatus},
};

type Event = crate::Event<Test>;

fn collateral(amount: u128) -> Amount<Test> {
	Amount::new(amount, DEFAULT_COLLATERAL_CURRENCY)
}
fn griefing(amount: u128) -> Amount<Test> {
	Amount::new(amount, DEFAULT_NATIVE_CURRENCY)
}
fn wrapped(amount: u128) -> Amount<Test> {
	Amount::new(amount, DEFAULT_WRAPPED_CURRENCY)
}

macro_rules! assert_emitted {
	($event:expr) => {
		let test_event = TestEvent::Redeem($event);
		assert!(System::events().iter().any(|a| a.event == test_event));
	};
	($event:expr, $times:expr) => {
		let test_event = TestEvent::Redeem($event);
		assert_eq!(System::events().iter().filter(|a| a.event == test_event).count(), $times);
	};
}

fn inject_redeem_request(
	key: H256,
	value: RedeemRequest<AccountId, BlockNumber, Balance, CurrencyId>,
) {
	Redeem::insert_redeem_request(&key, &value)
}

fn default_vault() -> DefaultVault<Test> {
	vault_registry::Vault {
		id: VAULT,
		to_be_replaced_tokens: 0,
		to_be_issued_tokens: 0,
		issued_tokens: 10,
		replace_collateral: 0,
		to_be_redeemed_tokens: 0,
		active_replace_collateral: 0,
		banned_until: None,
		secure_collateral_threshold: None,
		status: VaultStatus::Active(true),
		liquidated_collateral: 0,
	}
}

#[test]
fn test_request_redeem_fails_with_amount_exceeds_user_balance() {
	run_test(|| {
		let amount = Amount::<Test>::new(2, get_wrapped_currency_id());
		amount.mint_to(&USER).unwrap();
		let amount = 10_000_000;
		assert_err!(
			Redeem::request_redeem(
				RuntimeOrigin::signed(USER),
				amount,
				RANDOM_STELLAR_PUBLIC_KEY,
				VAULT
			),
			TestError::AmountExceedsUserBalance
		);
	})
}

#[test]
fn test_request_redeem_fails_with_amount_below_minimum() {
	run_test(|| {
		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 10,
				replace_collateral: 0,
				to_be_redeemed_tokens: 0,
				active_replace_collateral: 0,
				banned_until: None,
				secure_collateral_threshold: None,
				status: VaultStatus::Active(true),
				liquidated_collateral: 0,
			},
		);

		let redeemer = USER;
		let amount = 9;

		ext::vault_registry::try_increase_to_be_redeemed_tokens::<Test>.mock_safe(
			move |vault_id, amount_wrapped| {
				assert_eq!(vault_id, &VAULT);
				assert_eq!(amount_wrapped, &wrapped(amount));

				MockResult::Return(Ok(()))
			},
		);

		assert_err!(
			Redeem::request_redeem(
				RuntimeOrigin::signed(redeemer),
				1,
				RANDOM_STELLAR_PUBLIC_KEY,
				VAULT
			),
			TestError::AmountBelowMinimumTransferAmount
		);
	})
}

#[test]
fn test_request_redeem_fails_with_vault_not_found() {
	run_test(|| {
		assert_err!(
			Redeem::request_redeem(
				RuntimeOrigin::signed(USER),
				1500,
				RANDOM_STELLAR_PUBLIC_KEY,
				VAULT
			),
			VaultRegistryError::VaultNotFound
		);
	})
}

#[test]
fn test_request_redeem_fails_with_vault_banned() {
	run_test(|| {
		ext::vault_registry::ensure_not_banned::<Test>
			.mock_safe(|_| MockResult::Return(Err(VaultRegistryError::VaultBanned.into())));

		assert_err!(
			Redeem::request_redeem(
				RuntimeOrigin::signed(USER),
				1500,
				RANDOM_STELLAR_PUBLIC_KEY,
				VAULT
			),
			VaultRegistryError::VaultBanned
		);
	})
}

#[test]
fn test_request_redeem_fails_with_vault_liquidated() {
	run_test(|| {
		ext::vault_registry::ensure_not_banned::<Test>.mock_safe(|_| MockResult::Return(Ok(())));
		assert_err!(
			Redeem::request_redeem(
				RuntimeOrigin::signed(USER),
				3000,
				RANDOM_STELLAR_PUBLIC_KEY,
				VAULT
			),
			VaultRegistryError::VaultNotFound
		);
	})
}

#[test]
fn test_request_redeem_succeeds_with_normal_redeem() {
	run_test(|| {
		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 10,
				to_be_redeemed_tokens: 0,
				replace_collateral: 0,
				active_replace_collateral: 0,
				banned_until: None,
				secure_collateral_threshold: None,
				status: VaultStatus::Active(true),
				liquidated_collateral: 0,
			},
		);

		let redeemer = USER;
		let amount = 90;
		let asset = DEFAULT_WRAPPED_CURRENCY;
		let redeem_fee = 5;
		let stellar_address = RANDOM_STELLAR_PUBLIC_KEY;

		ext::vault_registry::try_increase_to_be_redeemed_tokens::<Test>.mock_safe(
			move |vault_id, amount_wrapped| {
				assert_eq!(vault_id, &VAULT);
				assert_eq!(amount_wrapped, &wrapped(amount - redeem_fee));

				MockResult::Return(Ok(()))
			},
		);

		Amount::<Test>::lock_on.mock_safe(move |amount_wrapped, account| {
			assert_eq!(account, &redeemer);
			assert_eq!(amount_wrapped, &wrapped(amount));

			MockResult::Return(Ok(()))
		});

		ext::security::get_secure_id::<Test>.mock_safe(move |_| MockResult::Return(H256([0; 32])));
		ext::vault_registry::is_vault_below_premium_threshold::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(false)));
		ext::fee::get_redeem_fee::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(wrapped(redeem_fee))));
		let transfer_fee = Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY).unwrap();

		assert_ok!(Redeem::request_redeem(
			RuntimeOrigin::signed(redeemer),
			amount,
			stellar_address,
			VAULT
		));

		assert_emitted!(Event::RequestRedeem {
			redeem_id: H256([0; 32]),
			redeemer,
			amount: amount - redeem_fee - transfer_fee.amount(),
			asset,
			fee: redeem_fee,
			premium: 0,
			vault_id: VAULT,
			stellar_address,
			transfer_fee: Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY)
				.unwrap()
				.amount()
		});
		assert_ok!(
			Redeem::get_open_redeem_request_from_id(&H256([0; 32])),
			RedeemRequest {
				period: Redeem::redeem_period(),
				vault: VAULT,
				opentime: 1,
				fee: redeem_fee,
				amount: amount - redeem_fee - transfer_fee.amount(),
				asset,
				premium: 0,
				redeemer,
				stellar_address,
				status: RedeemRequestStatus::Pending,
				transfer_fee: Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY)
					.unwrap()
					.amount(),
			}
		);
	})
}

#[test]
fn test_request_redeem_succeeds_with_self_redeem() {
	run_test(|| {
		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 10,
				to_be_redeemed_tokens: 0,
				active_replace_collateral: 0,
				replace_collateral: 0,
				banned_until: None,
				secure_collateral_threshold: None,
				status: VaultStatus::Active(true),
				liquidated_collateral: 0,
			},
		);

		let redeemer = VAULT.account_id;
		let amount = 90;
		let asset = DEFAULT_WRAPPED_CURRENCY;
		let stellar_address = RANDOM_STELLAR_PUBLIC_KEY;

		ext::vault_registry::try_increase_to_be_redeemed_tokens::<Test>.mock_safe(
			move |vault_id, amount_wrapped| {
				assert_eq!(vault_id, &VAULT);
				assert_eq!(amount_wrapped, &wrapped(amount));

				MockResult::Return(Ok(()))
			},
		);

		Amount::<Test>::lock_on.mock_safe(move |amount_wrapped, account| {
			assert_eq!(account, &redeemer);
			assert_eq!(amount_wrapped, &wrapped(amount));

			MockResult::Return(Ok(()))
		});

		ext::security::get_secure_id::<Test>.mock_safe(move |_| MockResult::Return(H256::zero()));
		ext::vault_registry::is_vault_below_premium_threshold::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(false)));
		let transfer_fee = Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY).unwrap();

		assert_ok!(Redeem::request_redeem(
			RuntimeOrigin::signed(redeemer),
			amount,
			stellar_address,
			VAULT
		));

		assert_emitted!(Event::RequestRedeem {
			redeem_id: H256::zero(),
			redeemer,
			amount: amount - transfer_fee.amount(),
			asset,
			fee: 0,
			premium: 0,
			vault_id: VAULT,
			stellar_address,
			transfer_fee: Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY)
				.unwrap()
				.amount()
		});
		assert_ok!(
			Redeem::get_open_redeem_request_from_id(&H256::zero()),
			RedeemRequest {
				period: Redeem::redeem_period(),
				vault: VAULT,
				opentime: 1,
				fee: 0,
				amount: amount - transfer_fee.amount(),
				asset,
				premium: 0,
				redeemer,
				stellar_address,
				status: RedeemRequestStatus::Pending,
				transfer_fee: Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY)
					.unwrap()
					.amount(),
			}
		);
	})
}

#[test]
fn test_liquidation_redeem_succeeds() {
	run_test(|| {
		let total_amount = 10 * 100_000_000;

		ext::treasury::get_balance::<Test>
			.mock_safe(move |_, _| MockResult::Return(wrapped(total_amount)));

		Amount::<Test>::lock_on.mock_safe(move |_, _| MockResult::Return(Ok(())));
		Amount::<Test>::burn_from.mock_safe(move |amount, redeemer_id| {
			assert_eq!(redeemer_id, &USER);
			assert_eq!(amount, &wrapped(total_amount));

			MockResult::Return(Ok(()))
		});

		ext::vault_registry::redeem_tokens_liquidation::<Test>.mock_safe(
			move |_, redeemer_id, amount| {
				assert_eq!(redeemer_id, &USER);
				assert_eq!(amount, &wrapped(total_amount));

				MockResult::Return(Ok(()))
			},
		);

		assert_ok!(Redeem::liquidation_redeem(
			RuntimeOrigin::signed(USER),
			DEFAULT_CURRENCY_PAIR,
			total_amount,
		));
	})
}

#[test]
fn test_execute_redeem_fails_with_redeem_id_not_found() {
	run_test(|| {
		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		assert_err!(
			Redeem::execute_redeem(
				RuntimeOrigin::signed(VAULT.account_id),
				H256([0u8; 32]),
				Vec::default(),
				Vec::default(),
				Vec::default(),
			),
			TestError::RedeemIdNotFound
		);
	})
}

#[test]
fn test_execute_redeem_succeeds_with_another_account() {
	run_test(|| {
		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		Security::<Test>::set_active_block_number(40);
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 200,
				to_be_redeemed_tokens: 200,
				replace_collateral: 0,
				active_replace_collateral: 0,
				banned_until: None,
				secure_collateral_threshold: None,
				status: VaultStatus::Active(true),
				liquidated_collateral: 0,
			},
		);
		ext::stellar_relay::ensure_transaction_memo_matches_hash::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(())));
		ext::stellar_relay::validate_stellar_transaction::<Test>
			.mock_safe(move |_, _, _| MockResult::Return(Ok(())));

		let transfer_fee = Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY).unwrap();

		inject_redeem_request(
			H256([0u8; 32]),
			RedeemRequest {
				period: 0,
				vault: VAULT,
				opentime: 40,
				fee: 0,
				amount: 100,
				asset: DEFAULT_WRAPPED_CURRENCY,
				premium: 0,
				redeemer: USER,
				stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
				status: RedeemRequestStatus::Pending,
				transfer_fee: transfer_fee.amount(),
			},
		);

		Amount::<Test>::burn_from.mock_safe(move |amount_wrapped, redeemer| {
			assert_eq!(redeemer, &USER);
			assert_eq!(amount_wrapped, &(wrapped(100) + transfer_fee));

			MockResult::Return(Ok(()))
		});

		ext::vault_registry::redeem_tokens::<Test>.mock_safe(
			move |vault, amount_wrapped, premium, _| {
				assert_eq!(vault, &VAULT);
				assert_eq!(amount_wrapped, &(wrapped(100) + transfer_fee));
				assert_eq!(premium, &collateral(0));

				MockResult::Return(Ok(()))
			},
		);

		let (
			transaction_envelope_xdr_encoded,
			scp_envelopes_xdr_encoded,
			transaction_set_xdr_encoded,
		) = stellar_relay::testing_utils::create_dummy_scp_structs_encoded();

		assert_ok!(Redeem::execute_redeem(
			RuntimeOrigin::signed(USER),
			H256([0u8; 32]),
			transaction_envelope_xdr_encoded,
			scp_envelopes_xdr_encoded,
			transaction_set_xdr_encoded,
		));
		assert_emitted!(Event::ExecuteRedeem {
			redeem_id: H256([0; 32]),
			redeemer: USER,
			vault_id: VAULT,
			amount: 100,
			asset: DEFAULT_WRAPPED_CURRENCY,
			fee: 0,
			transfer_fee: transfer_fee.amount(),
		});
		assert_err!(
			Redeem::get_open_redeem_request_from_id(&H256([0u8; 32])),
			TestError::RedeemCompleted,
		);
	})
}

#[test]
fn test_execute_redeem_succeeds() {
	run_test(|| {
		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		Security::<Test>::set_active_block_number(40);
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 200,
				to_be_redeemed_tokens: 200,
				replace_collateral: 0,
				active_replace_collateral: 0,
				banned_until: None,
				secure_collateral_threshold: None,
				status: VaultStatus::Active(true),
				liquidated_collateral: 0,
			},
		);
		ext::stellar_relay::ensure_transaction_memo_matches_hash::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(())));
		ext::stellar_relay::validate_stellar_transaction::<Test>
			.mock_safe(move |_, _, _| MockResult::Return(Ok(())));

		let transfer_fee = Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY).unwrap();

		inject_redeem_request(
			H256([0u8; 32]),
			RedeemRequest {
				period: 0,
				vault: VAULT,
				opentime: 40,
				fee: 0,
				amount: 100,
				asset: DEFAULT_WRAPPED_CURRENCY,
				premium: 0,
				redeemer: USER,
				stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
				status: RedeemRequestStatus::Pending,
				transfer_fee: transfer_fee.amount(),
			},
		);

		Amount::<Test>::burn_from.mock_safe(move |amount_wrapped, redeemer| {
			assert_eq!(redeemer, &USER);
			assert_eq!(amount_wrapped, &(wrapped(100) + transfer_fee));

			MockResult::Return(Ok(()))
		});

		ext::vault_registry::redeem_tokens::<Test>.mock_safe(
			move |vault, amount_wrapped, premium, _| {
				assert_eq!(vault, &VAULT);
				assert_eq!(amount_wrapped, &(wrapped(100) + transfer_fee));
				assert_eq!(premium, &collateral(0));

				MockResult::Return(Ok(()))
			},
		);
		let op_amount = 100 - (transfer_fee.amount() as i64);
		let op = get_operation(op_amount, RANDOM_STELLAR_PUBLIC_KEY);
		let (
			transaction_envelope_xdr_encoded,
			scp_envelopes_xdr_encoded,
			transaction_set_xdr_encoded,
		) = stellar_relay::testing_utils::create_dummy_scp_structs_with_operation_encoded(op);

		assert_ok!(Redeem::execute_redeem(
			RuntimeOrigin::signed(VAULT.account_id),
			H256([0u8; 32]),
			transaction_envelope_xdr_encoded,
			scp_envelopes_xdr_encoded,
			transaction_set_xdr_encoded
		));
		assert_emitted!(Event::ExecuteRedeem {
			redeem_id: H256([0; 32]),
			redeemer: USER,
			vault_id: VAULT,
			amount: 100,
			asset: DEFAULT_WRAPPED_CURRENCY,
			fee: 0,
			transfer_fee: transfer_fee.amount(),
		});
		assert_err!(
			Redeem::get_open_redeem_request_from_id(&H256([0u8; 32])),
			TestError::RedeemCompleted,
		);
	})
}

fn get_operation(amount: i64, stellar_address: [u8; 32]) -> Operation {
	let alpha_num4 = AlphaNum4 {
		asset_code: *b"USDC",
		issuer: PublicKey::PublicKeyTypeEd25519([
			20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46,
			199, 108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
		]),
	};
	let stellar_asset = Asset::AssetTypeCreditAlphanum4(alpha_num4);
	let amount = StroopAmount(amount);
	let address = PublicKey::PublicKeyTypeEd25519(stellar_address);
	let op =
		Operation::new_payment(address, stellar_asset, amount).expect("Should create operation");
	op
}

#[test]
fn test_cancel_redeem_fails_with_redeem_id_not_found() {
	run_test(|| {
		assert_err!(
			Redeem::cancel_redeem(RuntimeOrigin::signed(USER), H256([0u8; 32]), false),
			TestError::RedeemIdNotFound
		);
	})
}

#[test]
fn test_cancel_redeem_fails_with_time_not_expired() {
	run_test(|| {
		Security::<Test>::set_active_block_number(10);

		Redeem::get_open_redeem_request_from_id.mock_safe(|_| {
			MockResult::Return(Ok(RedeemRequest {
				period: 0,
				vault: VAULT,
				opentime: 0,
				fee: 0,
				amount: 0,
				asset: DEFAULT_WRAPPED_CURRENCY,
				premium: 0,
				redeemer: USER,
				stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
				status: RedeemRequestStatus::Pending,
				transfer_fee: Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY)
					.unwrap()
					.amount(),
			}))
		});

		assert_err!(
			Redeem::cancel_redeem(RuntimeOrigin::signed(USER), H256([0u8; 32]), false),
			TestError::TimeNotExpired
		);
	})
}

#[test]
fn test_cancel_redeem_fails_with_unauthorized_caller() {
	run_test(|| {
		Security::<Test>::set_active_block_number(20);

		Redeem::get_open_redeem_request_from_id.mock_safe(|_| {
			MockResult::Return(Ok(RedeemRequest {
				period: 0,
				vault: VAULT,
				opentime: 0,
				fee: 0,
				amount: 0,
				asset: DEFAULT_WRAPPED_CURRENCY,
				premium: 0,
				redeemer: USER,
				stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
				status: RedeemRequestStatus::Pending,
				transfer_fee: Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY)
					.unwrap()
					.amount(),
			}))
		});

		assert_noop!(
			Redeem::cancel_redeem(RuntimeOrigin::signed(CAROL), H256([0u8; 32]), true),
			TestError::UnauthorizedRedeemer
		);
	})
}

#[test]
fn test_cancel_redeem_succeeds() {
	run_test(|| {
		inject_redeem_request(
			H256([0u8; 32]),
			RedeemRequest {
				period: 0,
				vault: VAULT,
				opentime: 10,
				fee: 0,
				amount: 10,
				asset: DEFAULT_WRAPPED_CURRENCY,
				premium: 0,
				redeemer: USER,
				stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
				status: RedeemRequestStatus::Pending,
				transfer_fee: Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY)
					.unwrap()
					.amount(),
			},
		);

		ext::security::parachain_block_expired::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok(true)));

		ext::vault_registry::ban_vault::<Test>.mock_safe(move |vault| {
			assert_eq!(vault, &VAULT);
			MockResult::Return(Ok(()))
		});
		Amount::<Test>::unlock_on.mock_safe(|_, _| MockResult::Return(Ok(())));
		ext::vault_registry::transfer_funds_saturated::<Test>
			.mock_safe(move |_, _, amount| MockResult::Return(Ok(*amount)));
		ext::vault_registry::get_vault_from_id::<Test>.mock_safe(|_| {
			MockResult::Return(Ok(vault_registry::types::Vault {
				status: VaultStatus::Active(true),
				..vault_registry::types::Vault::new(VAULT)
			}))
		});
		ext::vault_registry::decrease_to_be_redeemed_tokens::<Test>
			.mock_safe(|_, _| MockResult::Return(Ok(())));
		assert_ok!(Redeem::cancel_redeem(RuntimeOrigin::signed(USER), H256([0u8; 32]), false));
		assert_err!(
			Redeem::get_open_redeem_request_from_id(&H256([0u8; 32])),
			TestError::RedeemCancelled,
		);
		assert_emitted!(Event::CancelRedeem {
			redeem_id: H256([0; 32]),
			redeemer: USER,
			vault_id: VAULT,
			slashed_amount: 1,
			status: RedeemRequestStatus::Retried
		});
	})
}

#[test]
fn test_mint_tokens_for_reimbursed_redeem() {
	// PRECONDITION: The vault MUST NOT be banned.
	// POSTCONDITION: `tryIncreaseToBeIssuedTokens` and `issueTokens` MUST be called,
	// both with the vault and `redeem.amount + redeem.transferFee` as arguments.
	run_test(|| {
		let redeem_request = RedeemRequest {
			period: 0,
			vault: VAULT,
			opentime: 40,
			fee: 0,
			amount: 100,
			asset: DEFAULT_WRAPPED_CURRENCY,
			premium: 0,
			redeemer: USER,
			stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
			status: RedeemRequestStatus::Reimbursed(false),
			transfer_fee: 1,
		};
		let redeem_request_clone = redeem_request.clone();
		inject_redeem_request(H256([0u8; 32]), redeem_request.clone());
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				banned_until: Some(100),
				status: VaultStatus::Active(true),
				..default_vault()
			},
		);
		Security::<Test>::set_active_block_number(100);
		assert_noop!(
			Redeem::mint_tokens_for_reimbursed_redeem(
				RuntimeOrigin::signed(VAULT.account_id),
				VAULT.currencies.clone(),
				H256([0u8; 32])
			),
			VaultRegistryError::ExceedingVaultLimit
		);
		Security::<Test>::set_active_block_number(101);
		ext::vault_registry::try_increase_to_be_issued_tokens::<Test>.mock_safe(
			move |vault_id, amount| {
				assert_eq!(vault_id, &VAULT);
				assert_eq!(amount, &wrapped(redeem_request.amount + redeem_request.transfer_fee));
				MockResult::Return(Ok(()))
			},
		);
		ext::vault_registry::issue_tokens::<Test>.mock_safe(move |vault_id, amount| {
			assert_eq!(vault_id, &VAULT);
			assert_eq!(
				amount,
				&wrapped(redeem_request_clone.amount + redeem_request_clone.transfer_fee)
			);
			MockResult::Return(Ok(()))
		});
		assert_ok!(Redeem::mint_tokens_for_reimbursed_redeem(
			RuntimeOrigin::signed(VAULT.account_id),
			VAULT.currencies.clone(),
			H256([0u8; 32])
		));
	});
}

#[test]
fn test_set_redeem_period_only_root() {
	run_test(|| {
		assert_noop!(
			Redeem::set_redeem_period(RuntimeOrigin::signed(USER), 1),
			DispatchError::BadOrigin
		);
		assert_ok!(Redeem::set_redeem_period(RuntimeOrigin::root(), 1));
	})
}

mod spec_based_tests {
	use super::*;

	#[test]
	fn test_request_reduces_to_be_replaced() {
		// POSTCONDITION: `decreaseToBeReplacedTokens` MUST be called, supplying `vault` and
		// `burnedTokens`. The returned `replaceCollateral` MUST be released by this function.
		run_test(|| {
			let amount_to_redeem = 100;
			let replace_collateral = 100;
			let amount = Amount::<Test>::new(amount_to_redeem, get_wrapped_currency_id());
			amount.mint_to(&USER).unwrap();
			let _asset = DEFAULT_WRAPPED_CURRENCY;

			ext::vault_registry::ensure_not_banned::<Test>
				.mock_safe(move |_vault_id| MockResult::Return(Ok(())));
			ext::vault_registry::try_increase_to_be_redeemed_tokens::<Test>
				.mock_safe(move |_vault_id, _amount| MockResult::Return(Ok(())));
			ext::vault_registry::is_vault_below_premium_threshold::<Test>
				.mock_safe(move |_vault_id| MockResult::Return(Ok(false)));
			let redeem_fee = Fee::get_redeem_fee(&wrapped(amount_to_redeem)).unwrap();
			let burned_tokens = wrapped(amount_to_redeem) - redeem_fee;

			ext::vault_registry::decrease_to_be_replaced_tokens::<Test>.mock_safe(
				move |vault_id, tokens| {
					assert_eq!(vault_id, &VAULT);
					assert_eq!(tokens, &burned_tokens);
					MockResult::Return(Ok((wrapped(0), griefing(0))))
				},
			);

			// The returned `replaceCollateral` MUST be released
			currency::Amount::unlock_on.mock_safe(move |collateral_amount, vault_id| {
				assert_eq!(vault_id, &VAULT.account_id);
				assert_eq!(collateral_amount, &collateral(replace_collateral));
				MockResult::Return(Ok(()))
			});

			assert_ok!(Redeem::request_redeem(
				RuntimeOrigin::signed(USER),
				amount_to_redeem,
				RANDOM_STELLAR_PUBLIC_KEY,
				VAULT
			));
		})
	}

	#[test]
	fn test_liquidation_redeem_succeeds() {
		run_test(|| {
			let total_amount = 10 * 100_000_000;

			ext::treasury::get_balance::<Test>
				.mock_safe(move |_, _| MockResult::Return(wrapped(total_amount)));

			Amount::<Test>::lock_on.mock_safe(move |_, _| MockResult::Return(Ok(())));
			Amount::<Test>::burn_from.mock_safe(move |amount, redeemer_id| {
				assert_eq!(redeemer_id, &USER);
				assert_eq!(amount, &wrapped(total_amount));

				MockResult::Return(Ok(()))
			});

			ext::vault_registry::redeem_tokens_liquidation::<Test>.mock_safe(
				move |_, redeemer_id, amount| {
					assert_eq!(redeemer_id, &USER);
					assert_eq!(amount, &wrapped(total_amount));

					MockResult::Return(Ok(()))
				},
			);

			assert_ok!(Redeem::liquidation_redeem(
				RuntimeOrigin::signed(USER),
				DEFAULT_CURRENCY_PAIR,
				total_amount,
			));
		})
	}

	#[test]
	fn test_execute_redeem_succeeds_with_another_account() {
		run_test(|| {
			convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
			Security::<Test>::set_active_block_number(40);
			<vault_registry::Pallet<Test>>::insert_vault(
				&VAULT,
				vault_registry::Vault {
					id: VAULT,
					to_be_replaced_tokens: 0,
					to_be_issued_tokens: 0,
					issued_tokens: 200,
					to_be_redeemed_tokens: 200,
					replace_collateral: 0,
					banned_until: None,
					status: VaultStatus::Active(true),
					..default_vault()
				},
			);
			ext::stellar_relay::ensure_transaction_memo_matches_hash::<Test>
				.mock_safe(move |_, _| MockResult::Return(Ok(())));
			ext::stellar_relay::validate_stellar_transaction::<Test>
				.mock_safe(move |_, _, _| MockResult::Return(Ok(())));

			let transfer_fee = Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY).unwrap();
			let redeem_request = RedeemRequest {
				period: 0,
				vault: VAULT,
				opentime: 40,
				fee: 0,
				amount: 100,
				asset: DEFAULT_WRAPPED_CURRENCY,
				premium: 0,
				redeemer: USER,
				stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
				status: RedeemRequestStatus::Pending,
				transfer_fee: transfer_fee.amount(),
			};
			inject_redeem_request(H256([0u8; 32]), redeem_request.clone());

			Amount::<Test>::burn_from.mock_safe(move |_, _| MockResult::Return(Ok(())));

			ext::vault_registry::redeem_tokens::<Test>.mock_safe(
				move |vault, amount_wrapped, premium, redeemer| {
					assert_eq!(vault, &redeem_request.vault);
					assert_eq!(
						amount_wrapped,
						&wrapped(redeem_request.amount + redeem_request.transfer_fee)
					);
					assert_eq!(premium, &collateral(redeem_request.premium));
					assert_eq!(redeemer, &redeem_request.redeemer);

					MockResult::Return(Ok(()))
				},
			);

			let (
				transaction_envelope_xdr_encoded,
				scp_envelopes_xdr_encoded,
				transaction_set_xdr_encoded,
			) = stellar_relay::testing_utils::create_dummy_scp_structs_encoded();

			assert_ok!(Redeem::execute_redeem(
				RuntimeOrigin::signed(USER),
				H256([0u8; 32]),
				transaction_envelope_xdr_encoded,
				scp_envelopes_xdr_encoded,
				transaction_set_xdr_encoded,
			));
			assert_emitted!(Event::ExecuteRedeem {
				redeem_id: H256([0; 32]),
				redeemer: USER,
				vault_id: VAULT,
				amount: 100,
				asset: DEFAULT_WRAPPED_CURRENCY,
				fee: 0,
				transfer_fee: transfer_fee.amount(),
			});
			assert_err!(
				Redeem::get_open_redeem_request_from_id(&H256([0u8; 32])),
				TestError::RedeemCompleted,
			);
		})
	}

	#[test]
	fn test_cancel_redeem_above_secure_threshold_succeeds() {
		// POSTCONDITIONS:
		// - If reimburse is true:
		//   - If after the loss of collateral the vault remains above the
		//     `SecureCollateralThreshold`:
		//       - `decreaseToBeRedeemedTokens` MUST be called, supplying the vault and
		//         amountIncludingParachainFee as arguments.
		run_test(|| {
			let redeem_request = RedeemRequest {
				period: 0,
				vault: VAULT,
				opentime: 10,
				fee: 0,
				amount: 10,
				asset: DEFAULT_WRAPPED_CURRENCY,
				premium: 0,
				redeemer: USER,
				stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
				status: RedeemRequestStatus::Pending,
				transfer_fee: Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY)
					.unwrap()
					.amount(),
			};
			inject_redeem_request(H256([0u8; 32]), redeem_request.clone());

			ext::security::parachain_block_expired::<Test>
				.mock_safe(|_, _| MockResult::Return(Ok(true)));
			ext::vault_registry::is_vault_below_secure_threshold::<Test>
				.mock_safe(|_| MockResult::Return(Ok(false)));
			ext::vault_registry::ban_vault::<Test>.mock_safe(move |vault| {
				assert_eq!(vault, &VAULT);
				MockResult::Return(Ok(()))
			});
			Amount::<Test>::unlock_on.mock_safe(|_, _| MockResult::Return(Ok(())));
			Amount::<Test>::transfer.mock_safe(|_, _, _| MockResult::Return(Ok(())));
			ext::vault_registry::transfer_funds_saturated::<Test>
				.mock_safe(move |_, _, amount| MockResult::Return(Ok(*amount)));
			ext::vault_registry::get_vault_from_id::<Test>.mock_safe(|_| {
				MockResult::Return(Ok(vault_registry::types::Vault {
					status: VaultStatus::Active(true),
					..default_vault()
				}))
			});
			ext::vault_registry::decrease_to_be_redeemed_tokens::<Test>.mock_safe(
				move |vault, amount| {
					assert_eq!(vault, &VAULT);
					assert_eq!(
						amount,
						&wrapped(redeem_request.amount + redeem_request.transfer_fee)
					);
					MockResult::Return(Ok(()))
				},
			);
			assert_ok!(Redeem::cancel_redeem(RuntimeOrigin::signed(USER), H256([0u8; 32]), true));
			assert_err!(
				Redeem::get_open_redeem_request_from_id(&H256([0u8; 32])),
				TestError::RedeemCancelled,
			);
			assert_emitted!(Event::CancelRedeem {
				redeem_id: H256([0; 32]),
				redeemer: USER,
				vault_id: VAULT,
				slashed_amount: 11,
				status: RedeemRequestStatus::Reimbursed(true)
			});
		})
	}

	#[test]
	fn test_cancel_redeem_below_secure_threshold_succeeds() {
		// POSTCONDITIONS:
		// - If reimburse is true:
		//   - If after the loss of collateral the vault is below the `SecureCollateralThreshold`:
		//       - `decreaseTokens` MUST be called, supplying the vault, the user, and
		//         amountIncludingParachainFee as arguments.
		run_test(|| {
			let redeem_request = RedeemRequest {
				period: 0,
				vault: VAULT,
				opentime: 10,
				fee: 0,
				amount: 10,
				asset: DEFAULT_WRAPPED_CURRENCY,
				premium: 0,
				redeemer: USER,
				stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
				status: RedeemRequestStatus::Pending,
				transfer_fee: Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY)
					.unwrap()
					.amount(),
			};
			inject_redeem_request(H256([0u8; 32]), redeem_request.clone());

			ext::security::parachain_block_expired::<Test>
				.mock_safe(|_, _| MockResult::Return(Ok(true)));
			ext::vault_registry::is_vault_below_secure_threshold::<Test>
				.mock_safe(|_| MockResult::Return(Ok(true)));
			ext::vault_registry::ban_vault::<Test>.mock_safe(move |vault| {
				assert_eq!(vault, &VAULT);
				MockResult::Return(Ok(()))
			});
			Amount::<Test>::unlock_on.mock_safe(|_, _| MockResult::Return(Ok(())));
			Amount::<Test>::burn_from.mock_safe(|_, _| MockResult::Return(Ok(())));
			ext::vault_registry::transfer_funds_saturated::<Test>
				.mock_safe(move |_, _, amount| MockResult::Return(Ok(*amount)));
			ext::vault_registry::get_vault_from_id::<Test>.mock_safe(|_| {
				MockResult::Return(Ok(vault_registry::types::Vault {
					status: VaultStatus::Active(true),
					..default_vault()
				}))
			});
			ext::vault_registry::decrease_tokens::<Test>.mock_safe(move |vault, user, amount| {
				assert_eq!(vault, &VAULT);
				assert_eq!(user, &USER);
				assert_eq!(amount, &wrapped(redeem_request.amount + redeem_request.transfer_fee));
				MockResult::Return(Ok(()))
			});
			assert_ok!(Redeem::cancel_redeem(RuntimeOrigin::signed(USER), H256([0u8; 32]), true));
			assert_err!(
				Redeem::get_open_redeem_request_from_id(&H256([0u8; 32])),
				TestError::RedeemCancelled,
			);
			assert_emitted!(Event::CancelRedeem {
				redeem_id: H256([0; 32]),
				redeemer: USER,
				vault_id: VAULT,
				slashed_amount: 11,
				status: RedeemRequestStatus::Reimbursed(false)
			});
		})
	}
}

#[test]
fn test_request_redeem_fails_limits() {
	run_test(|| {
		let volume_limit: u128 = 9;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			DEFAULT_COLLATERAL_CURRENCY,
			7200u64,
		);

		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 10,
				to_be_redeemed_tokens: 0,
				replace_collateral: 0,
				active_replace_collateral: 0,
				banned_until: None,
				secure_collateral_threshold: None,
				status: VaultStatus::Active(true),
				liquidated_collateral: 0,
			},
		);

		let redeemer = USER;
		let amount = volume_limit + 1;
		let redeem_fee = 5;
		let stellar_address = RANDOM_STELLAR_PUBLIC_KEY;

		ext::vault_registry::try_increase_to_be_redeemed_tokens::<Test>.mock_safe(
			move |vault_id, amount_wrapped| {
				assert_eq!(vault_id, &VAULT);
				assert_eq!(amount_wrapped, &wrapped(amount - redeem_fee));

				MockResult::Return(Ok(()))
			},
		);

		Amount::<Test>::lock_on.mock_safe(move |amount_wrapped, account| {
			assert_eq!(account, &redeemer);
			assert_eq!(amount_wrapped, &wrapped(amount));

			MockResult::Return(Ok(()))
		});

		ext::security::get_secure_id::<Test>.mock_safe(move |_| MockResult::Return(H256([0; 32])));
		ext::vault_registry::is_vault_below_premium_threshold::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(false)));
		ext::fee::get_redeem_fee::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(wrapped(redeem_fee))));

		assert_err!(
			Redeem::request_redeem(RuntimeOrigin::signed(redeemer), amount, stellar_address, VAULT),
			TestError::ExceedLimitVolumeForIssueRequest
		);
	})
}

#[test]
fn test_request_redeem_limits_succeeds() {
	run_test(|| {
		let volume_limit: u128 = 90u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			DEFAULT_COLLATERAL_CURRENCY,
			7200u64,
		);

		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 10,
				to_be_redeemed_tokens: 0,
				replace_collateral: 0,
				active_replace_collateral: 0,
				banned_until: None,
				secure_collateral_threshold: None,
				status: VaultStatus::Active(true),
				liquidated_collateral: 0,
			},
		);

		let redeemer = USER;
		let amount = volume_limit;
		let redeem_fee = 5;
		let stellar_address = RANDOM_STELLAR_PUBLIC_KEY;

		ext::vault_registry::try_increase_to_be_redeemed_tokens::<Test>.mock_safe(
			move |vault_id, amount_wrapped| {
				assert_eq!(vault_id, &VAULT);
				assert_eq!(amount_wrapped, &wrapped(amount - redeem_fee));

				MockResult::Return(Ok(()))
			},
		);

		Amount::<Test>::lock_on.mock_safe(move |amount_wrapped, account| {
			assert_eq!(account, &redeemer);
			assert_eq!(amount_wrapped, &wrapped(amount));

			MockResult::Return(Ok(()))
		});

		ext::security::get_secure_id::<Test>.mock_safe(move |_| MockResult::Return(H256([0; 32])));
		ext::vault_registry::is_vault_below_premium_threshold::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(false)));
		ext::fee::get_redeem_fee::<Test>
			.mock_safe(move |_| MockResult::Return(Ok(wrapped(redeem_fee))));

		assert_ok!(Redeem::request_redeem(
			RuntimeOrigin::signed(redeemer),
			amount,
			stellar_address,
			VAULT
		));
	})
}

#[test]
fn test_execute_redeem_within_rate_limit_succeeds() {
	run_test(|| {
		let volume_limit: u128 = 100u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			DEFAULT_COLLATERAL_CURRENCY,
			7200u64,
		);

		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		Security::<Test>::set_active_block_number(40);
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 200,
				to_be_redeemed_tokens: 200,
				replace_collateral: 0,
				banned_until: None,
				status: VaultStatus::Active(true),
				..default_vault()
			},
		);
		ext::stellar_relay::ensure_transaction_memo_matches_hash::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(())));
		ext::stellar_relay::validate_stellar_transaction::<Test>
			.mock_safe(move |_, _, _| MockResult::Return(Ok(())));

		let tranfer_fee = Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY).unwrap();
		let redeem_request = RedeemRequest {
			period: 0,
			vault: VAULT,
			opentime: 40,
			fee: 0,
			amount: volume_limit,
			asset: DEFAULT_WRAPPED_CURRENCY,
			premium: 0,
			redeemer: USER,
			stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
			status: RedeemRequestStatus::Pending,
			transfer_fee: tranfer_fee.amount(),
		};
		inject_redeem_request(H256([0u8; 32]), redeem_request.clone());

		Amount::<Test>::burn_from.mock_safe(move |_, _| MockResult::Return(Ok(())));

		ext::vault_registry::redeem_tokens::<Test>.mock_safe(
			move |vault, amount_wrapped, premium, redeemer| {
				assert_eq!(vault, &redeem_request.vault);
				assert_eq!(
					amount_wrapped,
					&wrapped(redeem_request.amount + redeem_request.transfer_fee)
				);
				assert_eq!(premium, &collateral(redeem_request.premium));
				assert_eq!(redeemer, &redeem_request.redeemer);

				MockResult::Return(Ok(()))
			},
		);

		let (
			transaction_envelope_xdr_encoded,
			scp_envelopes_xdr_encoded,
			transaction_set_xdr_encoded,
		) = stellar_relay::testing_utils::create_dummy_scp_structs_encoded();

		assert_ok!(Redeem::execute_redeem(
			RuntimeOrigin::signed(USER),
			H256([0u8; 32]),
			transaction_envelope_xdr_encoded,
			scp_envelopes_xdr_encoded,
			transaction_set_xdr_encoded,
		));
		assert_emitted!(Event::ExecuteRedeem {
			redeem_id: H256([0; 32]),
			redeemer: USER,
			vault_id: VAULT,
			amount: 100,
			asset: DEFAULT_WRAPPED_CURRENCY,
			fee: 0,
			transfer_fee: tranfer_fee.amount(),
		});
		assert_err!(
			Redeem::get_open_redeem_request_from_id(&H256([0u8; 32])),
			TestError::RedeemCompleted,
		);
	})
}

#[test]
fn test_execute_redeem_fails_when_exceeds_rate_limit() {
	run_test(|| {
		let volume_limit: u128 = 100u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			DEFAULT_COLLATERAL_CURRENCY,
			7200u64,
		);

		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		Security::<Test>::set_active_block_number(40);
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 200,
				to_be_redeemed_tokens: 200,
				replace_collateral: 0,
				banned_until: None,
				status: VaultStatus::Active(true),
				..default_vault()
			},
		);
		ext::stellar_relay::ensure_transaction_memo_matches_hash::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(())));
		ext::stellar_relay::validate_stellar_transaction::<Test>
			.mock_safe(move |_, _, _| MockResult::Return(Ok(())));

		let transfer_fee = Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY).unwrap();
		let amount = volume_limit;
		let redeem_request = RedeemRequest {
			period: 0,
			vault: VAULT,
			opentime: 40,
			fee: 0,
			amount,
			asset: DEFAULT_WRAPPED_CURRENCY,
			premium: 0,
			redeemer: USER,
			stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
			status: RedeemRequestStatus::Pending,
			transfer_fee: transfer_fee.amount(),
		};
		inject_redeem_request(H256([0u8; 32]), redeem_request.clone());

		Amount::<Test>::burn_from.mock_safe(move |_, _| MockResult::Return(Ok(())));

		ext::vault_registry::redeem_tokens::<Test>.mock_safe(
			move |vault, amount_wrapped, premium, redeemer| {
				assert_eq!(vault, &redeem_request.vault);
				assert_eq!(
					amount_wrapped,
					&wrapped(redeem_request.amount + redeem_request.transfer_fee)
				);
				assert_eq!(premium, &collateral(redeem_request.premium));
				assert_eq!(redeemer, &redeem_request.redeemer);

				MockResult::Return(Ok(()))
			},
		);

		let (
			transaction_envelope_xdr_encoded,
			scp_envelopes_xdr_encoded,
			transaction_set_xdr_encoded,
		) = stellar_relay::testing_utils::create_dummy_scp_structs_encoded();

		assert_ok!(Redeem::execute_redeem(
			RuntimeOrigin::signed(USER),
			H256([0u8; 32]),
			transaction_envelope_xdr_encoded,
			scp_envelopes_xdr_encoded,
			transaction_set_xdr_encoded,
		));
		assert_emitted!(Event::ExecuteRedeem {
			redeem_id: H256([0; 32]),
			redeemer: USER,
			vault_id: VAULT,
			amount: 100,
			asset: DEFAULT_WRAPPED_CURRENCY,
			fee: 0,
			transfer_fee: transfer_fee.amount(),
		});
		assert_err!(
			Redeem::get_open_redeem_request_from_id(&H256([0u8; 32])),
			TestError::RedeemCompleted,
		);

		let redeemer = USER;
		// Requesting a second redeem request with just 1 unit of collateral should fail because we
		// reached the volume limit already
		let amount = 1;
		let stellar_address = RANDOM_STELLAR_PUBLIC_KEY;
		assert_err!(
			Redeem::request_redeem(RuntimeOrigin::signed(redeemer), amount, stellar_address, VAULT),
			TestError::ExceedLimitVolumeForIssueRequest
		);
	})
}

#[test]
fn test_execute_redeem_after_rate_limit_interval_reset_succeeds() {
	run_test(|| {
		let volume_limit: u128 = 50u128;
		crate::Pallet::<Test>::_rate_limit_update(
			std::option::Option::<u128>::Some(volume_limit),
			DEFAULT_COLLATERAL_CURRENCY,
			7200u64,
		);

		convert_to.mock_safe(|_, x| MockResult::Return(Ok(x)));
		Security::<Test>::set_active_block_number(40);
		<vault_registry::Pallet<Test>>::insert_vault(
			&VAULT,
			vault_registry::Vault {
				id: VAULT,
				to_be_replaced_tokens: 0,
				to_be_issued_tokens: 0,
				issued_tokens: 400,
				to_be_redeemed_tokens: 200,
				replace_collateral: 0,
				banned_until: None,
				status: VaultStatus::Active(true),
				..default_vault()
			},
		);
		ext::stellar_relay::ensure_transaction_memo_matches_hash::<Test>
			.mock_safe(move |_, _| MockResult::Return(Ok(())));
		ext::stellar_relay::validate_stellar_transaction::<Test>
			.mock_safe(move |_, _, _| MockResult::Return(Ok(())));

		let transfer_fee = Redeem::get_current_inclusion_fee(DEFAULT_WRAPPED_CURRENCY).unwrap();
		let amount = volume_limit;
		let redeem_request = RedeemRequest {
			period: 0,
			vault: VAULT,
			opentime: 40,
			fee: 0,
			amount,
			asset: DEFAULT_WRAPPED_CURRENCY,
			premium: 0,
			redeemer: USER,
			stellar_address: RANDOM_STELLAR_PUBLIC_KEY,
			status: RedeemRequestStatus::Pending,
			transfer_fee: transfer_fee.amount(),
		};
		inject_redeem_request(H256([0u8; 32]), redeem_request.clone());

		Amount::<Test>::burn_from.mock_safe(move |_, _| MockResult::Return(Ok(())));

		ext::vault_registry::redeem_tokens::<Test>.mock_safe(
			move |vault, amount_wrapped, premium, redeemer| {
				assert_eq!(vault, &redeem_request.vault);
				assert_eq!(
					amount_wrapped,
					&wrapped(redeem_request.amount + redeem_request.transfer_fee)
				);
				assert_eq!(premium, &collateral(redeem_request.premium));
				assert_eq!(redeemer, &redeem_request.redeemer);

				MockResult::Return(Ok(()))
			},
		);

		let (
			transaction_envelope_xdr_encoded,
			scp_envelopes_xdr_encoded,
			transaction_set_xdr_encoded,
		) = stellar_relay::testing_utils::create_dummy_scp_structs_encoded();

		assert_ok!(Redeem::execute_redeem(
			RuntimeOrigin::signed(USER),
			H256([0u8; 32]),
			transaction_envelope_xdr_encoded,
			scp_envelopes_xdr_encoded,
			transaction_set_xdr_encoded,
		));
		assert_emitted!(Event::ExecuteRedeem {
			redeem_id: H256([0; 32]),
			redeemer: USER,
			vault_id: VAULT,
			amount,
			asset: DEFAULT_WRAPPED_CURRENCY,
			fee: 0,
			transfer_fee: transfer_fee.amount(),
		});
		assert_err!(
			Redeem::get_open_redeem_request_from_id(&H256([0u8; 32])),
			TestError::RedeemCompleted,
		);

		let redeemer = USER;
		let amount = 1;
		let stellar_address = RANDOM_STELLAR_PUBLIC_KEY;
		assert_err!(
			Redeem::request_redeem(RuntimeOrigin::signed(redeemer), amount, stellar_address, VAULT),
			TestError::ExceedLimitVolumeForIssueRequest
		);

		System::set_block_number(7200 + 20);

		// The volume limit does not automatically reset when the block number changes.
		// We need to request a new redeem so that the rate limit is reset for the new interval.
		assert!(<crate::CurrentVolumeAmount<Test>>::get() > BalanceOf::<Test>::zero());

		let redeemer = USER;
		let amount = volume_limit;
		let stellar_address = RANDOM_STELLAR_PUBLIC_KEY;
		assert_ok!(Redeem::request_redeem(
			RuntimeOrigin::signed(redeemer),
			amount,
			stellar_address,
			VAULT
		));

		// The volume limit should be reset after the new redeem request.
		// It is 0 because the redeem was only requested and not yet executed.
		assert_eq!(<crate::CurrentVolumeAmount<Test>>::get(), BalanceOf::<Test>::zero());
	})
}
