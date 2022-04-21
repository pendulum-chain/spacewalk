use crate::{currency::CurrencyId, mock::*};
use frame_support::{assert_ok, traits::fungibles::Inspect};

//stellar transaction that:
//sent 1 USDC (1_000_000_000_000)
//issued by (stellar: GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC) - (hex:
// 0x14d19631b03717d9ab9a366e10321ee266e72ec76cab6190f0a1336d48229f8b) from User (stellar:
// GDZWHHY5NNU2FA4LLBUZT36I6BYDUDHTEKVQCY6JUW4PGQF5GQLNH26I) - (hex:
// 0xf3639f1d6b69a2838b586999efc8f0703a0cf322ab0163c9a5b8f340bd3416d3) - (pendulum:
// 5HZq5zfRfUtZk8TMXNDe9jTeRAYN8VygLKX2Y2W9vipwxnNi) to Vault (stellar:
// GBTE4CYCGWD5SDQTZSTOQ44XQQC54QIQWQFWJOD4SHV7P524QW5MPRTZ) - (hex:
// 0x664e0b023587d90e13cca6e873978405de4110b40b64b87c91ebf7f75c85bac7)
const STELLAR_TRANSACTION_ENVELOPE: &str = "AAAAAgAAAADzY58da2mig4tYaZnvyPBwOgzzIqsBY8mluPNAvTQW0wAPQkAABtRJAAAABAAAAAEAAAAAAAAAAAAAAABiYIZ/AAAAAAAAAAEAAAAAAAAAAQAAAABmTgsCNYfZDhPMpuhzl4QF3kEQtAtkuHyR6/f3XIW6xwAAAAFVU0RDAAAAABTRljGwNxfZq5o2bhAyHuJm5y7HbKthkPChM21IIp+LAAAAAACYloAAAAAAAAAAAb00FtMAAABAgRd2oP9HKdjBal9hsiXjq2dQjpaxyJGXYVVQ8vQh1CLL0nzi19ZmR9NaDXqTOPswa7f+1mTCGQwwYRx3b+mvAg==";
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
const USDC: [u8; 4] = [b'U', b'S', b'D', b'C'];
const AMOUNT: u128 = 1_000_000_000_000;

#[test]
fn report_stellar_transaction_mints_asset() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_ok!(Spacewalk::report_stellar_transaction(
			Origin::signed([0; 32].into()),
			STELLAR_TRANSACTION_ENVELOPE.into()
		));
		//check user balance
		assert_eq!(
			Tokens::balance(
				CurrencyId::AlphaNum4 { code: USDC.into(), issuer: ISSUER },
				&USER.into()
			),
			AMOUNT
		);
	});
}

#[test]
fn redeem_burns_asset() {
	new_test_ext().execute_with(|| {
		//mint tokens for User
		assert_ok!(Spacewalk::report_stellar_transaction(
			Origin::signed([0; 32].into()),
			STELLAR_TRANSACTION_ENVELOPE.into()
		));
		//burn tokens
		assert_ok!(Spacewalk::redeem(
			Origin::signed(USER.into()),
			USDC.into(),
			ISSUER_STELLAR_ADDRESS.into(),
			AMOUNT,
			VAULT
		));
		//check user balance
		assert_eq!(
			Tokens::balance(
				CurrencyId::AlphaNum4 { code: USDC.into(), issuer: ISSUER },
				&USER.into()
			),
			0
		);
	});
}
