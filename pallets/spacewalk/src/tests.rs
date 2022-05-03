use crate::mock::*;

use frame_support::{assert_err, assert_ok, traits::fungibles::Inspect};
use sp_runtime::{DispatchError, ModuleError};

//stellar transaction that:
//sent 1 USDC (1_000_000_000_000)
//USDC issued by:
// - stellar: GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC
// - hex: 0x14d19631b03717d9ab9a366e10321ee266e72ec76cab6190f0a1336d48229f8b
//sent from User:
// - stellar: GDZWHHY5NNU2FA4LLBUZT36I6BYDUDHTEKVQCY6JUW4PGQF5GQLNH26I
// - hex: 0xf3639f1d6b69a2838b586999efc8f0703a0cf322ab0163c9a5b8f340bd3416d3
// - pendulum: 5HZq5zfRfUtZk8TMXNDe9jTeRAYN8VygLKX2Y2W9vipwxnNi
//to Vault:
// - stellar: GBTE4CYCGWD5SDQTZSTOQ44XQQC54QIQWQFWJOD4SHV7P524QW5MPRTZ
// - hex: 0x664e0b023587d90e13cca6e873978405de4110b40b64b87c91ebf7f75c85bac7
const STELLAR_TRANSACTION_ENVELOPE_USDC: &str = "AAAAAgAAAADzY58da2mig4tYaZnvyPBwOgzzIqsBY8mluPNAvTQW0wAPQkAABtRJAAAABAAAAAEAAAAAAAAAAAAAAABiYIZ/AAAAAAAAAAEAAAAAAAAAAQAAAABmTgsCNYfZDhPMpuhzl4QF3kEQtAtkuHyR6/f3XIW6xwAAAAFVU0RDAAAAABTRljGwNxfZq5o2bhAyHuJm5y7HbKthkPChM21IIp+LAAAAAACYloAAAAAAAAAAAb00FtMAAABAgRd2oP9HKdjBal9hsiXjq2dQjpaxyJGXYVVQ8vQh1CLL0nzi19ZmR9NaDXqTOPswa7f+1mTCGQwwYRx3b+mvAg==";
//same as USDC transaction above except sends 2 EUR (2_000_000_000_000) instead. (same issuer)
const STELLAR_TRANSACTION_ENVELOPE_EUR: &str = "AAAAAgAAAADzY58da2mig4tYaZnvyPBwOgzzIqsBY8mluPNAvTQW0wAPQkAABtRJAAAABQAAAAEAAAAAAAAAAAAAAABiYpU4AAAAAAAAAAEAAAAAAAAAAQAAAABmTgsCNYfZDhPMpuhzl4QF3kEQtAtkuHyR6/f3XIW6xwAAAAFFVVIAAAAAABTRljGwNxfZq5o2bhAyHuJm5y7HbKthkPChM21IIp+LAAAAAAExLQAAAAAAAAAAAb00FtMAAABALd+56WU3aFA5mNJMDTaQipGzqq3OUo4XhMi088glycFfvSJNo7rIzF4mc4mcxX5b7WB01P8mCEpVdn5wLOnHAg==";
const USDC_AMOUNT: u128 = 1_000_000_000_000;
const EUR_AMOUNT: u128 = 2_000_000_000_000;
const ISSUER_STELLAR_ADDRESS: &str = "GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC";
const ISSUER: [u8; 32] = [
	20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199, 108,
	171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
];
const USER: [u8; 32] = [
	243, 99, 159, 29, 107, 105, 162, 131, 139, 88, 105, 153, 239, 200, 240, 112, 58, 12, 243, 34,
	171, 1, 99, 201, 165, 184, 243, 64, 189, 52, 22, 211,
];
const VAULT: [u8; 32] = [
	102, 78, 11, 2, 53, 135, 217, 14, 19, 204, 166, 232, 115, 151, 132, 5, 222, 65, 16, 180, 11,
	100, 184, 124, 145, 235, 247, 247, 92, 133, 186, 199,
];
const USDC_CODE: [u8; 4] = [b'U', b'S', b'D', b'C'];
const EUR_CODE: [u8; 4] = [b'E', b'U', b'R', b'\0'];

#[test]
fn report_stellar_transaction_mints_usdc_asset() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_ok!(Spacewalk::report_stellar_transaction(
			Origin::signed([0; 32].into()),
			STELLAR_TRANSACTION_ENVELOPE_USDC.into()
		));
		//check user balance
		assert_eq!(
			Tokens::balance(
				CurrencyId::AlphaNum4 { code: USDC_CODE.into(), issuer: ISSUER },
				&USER.into()
			),
			USDC_AMOUNT
		);
	});
}

#[test]
fn report_stellar_transaction_mints_eur_asset() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_ok!(Spacewalk::report_stellar_transaction(
			Origin::signed([0; 32].into()),
			STELLAR_TRANSACTION_ENVELOPE_EUR.into()
		));
		//check user balance
		assert_eq!(
			Tokens::balance(
				CurrencyId::AlphaNum4 { code: EUR_CODE.into(), issuer: ISSUER },
				&USER.into()
			),
			EUR_AMOUNT
		);
	});
}

#[test]
fn report_stellar_transaction_invalid_envelope() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_err!(
			Spacewalk::report_stellar_transaction(
				Origin::signed([0; 32].into()),
				"INVALID_STELLAR_TRANSACTION_ENVELOPE".into()
			),
			DispatchError::Module(ModuleError {
				index: 6,
				error: 0,
				message: Some(&"XdrDecodingError")
			})
		);
	});
}

#[test]
fn redeem_burns_asset_usdc() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_ok!(Spacewalk::report_stellar_transaction(
			Origin::signed([0; 32].into()),
			STELLAR_TRANSACTION_ENVELOPE_USDC.into()
		));
		//burn tokens
		assert_ok!(Spacewalk::redeem(
			Origin::signed(USER.into()),
			USDC_CODE.into(),
			ISSUER_STELLAR_ADDRESS.into(),
			USDC_AMOUNT,
			VAULT
		));
		//check user balance
		assert_eq!(
			Tokens::balance(
				CurrencyId::AlphaNum4 { code: USDC_CODE.into(), issuer: ISSUER },
				&USER.into()
			),
			0
		);
	});
}

#[test]
fn redeem_burns_asset_eur() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_ok!(Spacewalk::report_stellar_transaction(
			Origin::signed([0; 32].into()),
			STELLAR_TRANSACTION_ENVELOPE_EUR.into()
		));
		//burn tokens
		assert_ok!(Spacewalk::redeem(
			Origin::signed(USER.into()),
			EUR_CODE.into(),
			ISSUER_STELLAR_ADDRESS.into(),
			EUR_AMOUNT,
			VAULT
		));
		//check user balance
		assert_eq!(
			Tokens::balance(
				CurrencyId::AlphaNum4 { code: EUR_CODE.into(), issuer: ISSUER },
				&USER.into()
			),
			0
		);
	});
}

#[test]
fn redeem_with_incorrect_caller() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_ok!(Spacewalk::report_stellar_transaction(
			Origin::signed([0; 32].into()),
			STELLAR_TRANSACTION_ENVELOPE_USDC.into()
		));
		//burn tokens
		assert_err!(
			Spacewalk::redeem(
				Origin::signed([0; 32].into()),
				USDC_CODE.into(),
				ISSUER_STELLAR_ADDRESS.into(),
				USDC_AMOUNT,
				VAULT
			),
			DispatchError::Module(ModuleError {
				index: 6,
				error: 2,
				message: Some(&"BalanceChangeError")
			})
		);
	});
}

#[test]
fn redeem_with_wrong_asset() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_ok!(Spacewalk::report_stellar_transaction(
			Origin::signed([0; 32].into()),
			STELLAR_TRANSACTION_ENVELOPE_USDC.into()
		));
		//burn tokens
		assert_err!(
			Spacewalk::redeem(
				Origin::signed(USER.into()),
				EUR_CODE.into(),
				ISSUER_STELLAR_ADDRESS.into(),
				USDC_AMOUNT,
				VAULT
			),
			DispatchError::Module(ModuleError {
				index: 6,
				error: 2,
				message: Some(&"BalanceChangeError")
			})
		);
	});
}

#[test]
fn redeem_with_wrong_issuer() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_ok!(Spacewalk::report_stellar_transaction(
			Origin::signed([0; 32].into()),

			STELLAR_TRANSACTION_ENVELOPE_USDC.into()
		));
		//burn tokens
		assert_err!(
			Spacewalk::redeem(
				Origin::signed(USER.into()),
				EUR_CODE.into(),
				[0; 32].into(),
				USDC_AMOUNT,
				VAULT
			),
			DispatchError::Module(ModuleError {
				index: 6,
				error: 1,
				message: Some(&"BalanceChangeError")
			})
		);
	});
}

#[test]
fn redeem_with_amount_too_high() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_ok!(Spacewalk::report_stellar_transaction(
			Origin::signed([0; 32].into()),

			STELLAR_TRANSACTION_ENVELOPE_USDC.into()
		));
		//burn tokens
		assert_err!(
			Spacewalk::redeem(
				Origin::signed(USER.into()),
				USDC_CODE.into(),
				ISSUER_STELLAR_ADDRESS.into(),
				USDC_AMOUNT * 2,
				VAULT
			),
			DispatchError::Module(ModuleError {
				index: 6,
				error: 2,
				message: Some(&"BalanceChangeError")
			})
		);
	});
}
