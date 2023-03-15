use substrate_stellar_sdk::{
	types::{AlphaNum12, AlphaNum4},
	Asset as StellarAsset, PublicKey,
};

use super::{CurrencyId, *};
use crate::CurrencyInfo;
use std::str::FromStr;

#[test]
fn test_from() {
	let account =
		PublicKey::from_encoding("GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC")
			.expect("invalid key encoding");
	let mut code_a4: [u8; 4] = [0; 4];
	code_a4.copy_from_slice("EURO".as_bytes());

	let currency_native = CurrencyId::from(StellarAsset::AssetTypeNative);
	assert_eq!(currency_native, CurrencyId::StellarNative);

	let currency_a4 = CurrencyId::from(StellarAsset::AssetTypeCreditAlphanum4(AlphaNum4 {
		asset_code: code_a4,
		issuer: account.clone(),
	}));
	assert_eq!(currency_a4, CurrencyId::AlphaNum4(code_a4, *account.as_binary()));

	let mut code_a12: [u8; 12] = [0; 12];
	code_a12.copy_from_slice("AmericaDolar".as_bytes());

	let currency_12 = CurrencyId::from(StellarAsset::AssetTypeCreditAlphanum12(AlphaNum12 {
		asset_code: code_a12,
		issuer: account.clone(),
	}));
	assert_eq!(currency_12, CurrencyId::AlphaNum12(code_a12, *account.as_binary()));
}

#[test]
fn test_try_from() {
	let account =
		PublicKey::from_encoding("GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC")
			.expect("invalid key encoding");
	let mut code_a4: [u8; 4] = [0; 4];
	code_a4.copy_from_slice("EURO".as_bytes());
	let mut code_a12: [u8; 12] = [0; 12];
	code_a12.copy_from_slice("AmericaDolar".as_bytes());

	let currency_a4 = CurrencyId::try_from(("EURO", (*account.as_binary()))).unwrap();
	assert_eq!(currency_a4, CurrencyId::AlphaNum4(code_a4, *account.as_binary()));

	let currency_a12 = CurrencyId::try_from(("AmericaDolar", (*account.as_binary()))).unwrap();
	assert_eq!(currency_a12, CurrencyId::AlphaNum12(code_a12, *account.as_binary()));
}

#[test]
fn test_try_into() {
	let account =
		PublicKey::from_encoding("GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC")
			.expect("invalid key encoding");
	let mut code_a4: [u8; 4] = [0; 4];
	code_a4.copy_from_slice("EURO".as_bytes());
	let mut code_a12: [u8; 12] = [0; 12];
	code_a12.copy_from_slice("AmericaDolar".as_bytes());

	let currency_a4: CurrencyId = StellarAsset::AssetTypeCreditAlphanum4(AlphaNum4 {
		asset_code: code_a4,
		issuer: account.clone(),
	})
	.try_into()
	.unwrap();
	assert_eq!(currency_a4, CurrencyId::AlphaNum4(code_a4, *account.as_binary()));

	let currency_a12: CurrencyId = StellarAsset::AssetTypeCreditAlphanum12(AlphaNum12 {
		asset_code: code_a12,
		issuer: account.clone(),
	})
	.try_into()
	.unwrap();
	assert_eq!(currency_a12, CurrencyId::AlphaNum12(code_a12, *account.as_binary()));
}

#[test]
fn test_currency_conversion_native() {
	let currency_id = CurrencyId::StellarNative;

	let currency_lookup = AssetConversion::lookup(currency_id);
	assert!(currency_lookup.is_ok());

	let currency_lookup = currency_lookup.unwrap();
	assert_eq!(currency_lookup, StellarAsset::AssetTypeNative);

	let lookup_orig = AssetConversion::unlookup(currency_lookup);
	assert_eq!(lookup_orig, currency_id);

	let currency_id = CurrencyId::XCM(0);
	assert!(AssetConversion::lookup(currency_id).is_err());
}

#[test]
fn test_currency_conversion_anum4() {
	let account =
		PublicKey::from_encoding("GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC")
			.unwrap();

	let mut code: [u8; 4] = [0; 4];
	code.copy_from_slice("EURO".as_bytes());

	let currency_id = CurrencyId::AlphaNum4(code, account.clone().into_binary());

	let currency_lookup = AssetConversion::lookup(currency_id);
	assert!(currency_lookup.is_ok());

	let currency_lookup = currency_lookup.unwrap();
	assert_eq!(
		currency_lookup,
		StellarAsset::AssetTypeCreditAlphanum4(AlphaNum4 { asset_code: code, issuer: account })
	);

	let lookup_orig = AssetConversion::unlookup(currency_lookup);
	assert_eq!(lookup_orig, currency_id);
}

#[test]
fn test_currency_conversion_anum12() {
	let account =
		PublicKey::from_encoding("GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC")
			.expect("invalid key encoding");

	let mut code: [u8; 12] = [0; 12];
	code.copy_from_slice("AmericaDolar".as_bytes());

	let currency_id = CurrencyId::AlphaNum12(code, account.clone().into_binary());

	let currency_lookup = AssetConversion::lookup(currency_id);
	assert!(currency_lookup.is_ok());

	let currency_lookup = currency_lookup.unwrap();
	assert_eq!(
		currency_lookup,
		StellarAsset::AssetTypeCreditAlphanum12(AlphaNum12 { asset_code: code, issuer: account })
	);

	let lookup_orig = AssetConversion::unlookup(currency_lookup);
	assert_eq!(lookup_orig, currency_id);
}

#[test]
fn test_balance_convr() {
	// One 'unit' on our chains
	let chain_unit = 1_000_000_000_000;
	// One 'unit' on the stellar chain
	let stellar_unit = 10_000_000;

	// We check if the conversion between a unit between both chains is correct
	let stellar_unit_result = BalanceConversion::lookup(chain_unit);
	assert!(stellar_unit_result.is_ok());
	assert_eq!(stellar_unit_result.unwrap(), stellar_unit);
	let chain_unit_result = BalanceConversion::unlookup(stellar_unit);
	assert_eq!(chain_unit_result, chain_unit);

	// We assume that the result is truncated and not rounded
	let chain_balance = 199_999;
	let truncated_result = BalanceConversion::lookup(chain_balance);
	assert!(truncated_result.is_ok());
	assert_eq!(truncated_result.unwrap(), 1);

	let negative_lookup = BalanceConversion::unlookup(i64::MIN);
	assert_eq!(negative_lookup, 0);

	// We check that the conversion fails if the number is too big and even the reduced value does
	// not fit into i64
	let failing_lookup = BalanceConversion::lookup(u128::MAX);
	assert!(failing_lookup.is_err());
}

#[test]
fn test_addr_conversion() {
	let account_id =
		AccountId32::from_str("5G9VdMwXvzza9pS8qE8ZHJk3CheHW9uucBn9ngW4C1gmmzpv").unwrap();

	let lookup_pk = AddressConversion::lookup(account_id.clone());
	assert!(lookup_pk.is_ok());

	let lookup_pk = lookup_pk.unwrap();
	let lookup_acc = AddressConversion::unlookup(lookup_pk);
	assert_eq!(lookup_acc, account_id);
}

#[test]
fn test_currencyid_one() {
	const USDC_ASSET: Asset = Asset::AlphaNum4 {
		code: *b"USDC",
		issuer: [
			20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46,
			199, 108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
		],
	};

	assert_eq!(USDC_ASSET.decimals(), Asset::StellarNative.decimals());
	assert_eq!(USDC_ASSET.one(), 10_000_000);
	assert_eq!(USDC_ASSET.one(), Asset::StellarNative.one());
}
