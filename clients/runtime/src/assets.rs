use std::convert::TryFrom;

use crate::{CurrencyId, Error};

/// Convert a ticker symbol into a `CurrencyId` at runtime
pub trait TryFromSymbol: Sized {
	fn try_from_symbol(symbol: String) -> Result<Self, Error>;
}

impl TryFromSymbol for CurrencyId {
	/// This can build a currency from a string in the following formats:
	/// - `XLM` which is simply the native Stellar currency
	/// - `<issuer>:<code>` where `<issuer>` is the issuer of the asset and `<code>` is the code of
	///   the asset
	/// - `<xcm_id>` where `<xcm_id>` is the XCM currency id
	fn try_from_symbol(symbol: String) -> Result<Self, Error> {
		let uppercase_symbol = symbol.to_uppercase();

		if uppercase_symbol.trim() == "XLM" {
			return Ok(CurrencyId::StellarNative)
		}

		// Try to build stellar asset
		let parts = uppercase_symbol.split(':').collect::<Vec<&str>>();
		if parts.len() == 2 {
			let issuer = parts[0].trim();
			let code = parts[1].trim();
			return CurrencyId::try_from((code, issuer)).map_err(|_| Error::InvalidCurrency)
		}

		// We assume that it is an XCM currency so we try to parse it as a number
		if let Ok(id) = uppercase_symbol.parse::<u8>() {
			Ok(CurrencyId::XCM(id))
		} else {
			Err(Error::InvalidCurrency)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use primitives::Asset;

	#[test]
	fn test_try_from_symbol_works_for_native_currency() {
		assert_eq!(
			CurrencyId::try_from_symbol("XLM".to_string()).expect("Conversion should work"),
			CurrencyId::StellarNative
		);
		assert_eq!(
			CurrencyId::try_from_symbol("xlm".to_string()).expect("Conversion should work"),
			CurrencyId::StellarNative
		);
		assert_eq!(
			CurrencyId::try_from_symbol("Xlm".to_string()).expect("Conversion should work"),
			CurrencyId::StellarNative
		);
	}

	#[test]
	fn test_try_from_symbol_works_for_stellar_asset() {
		let issuer_bytes = [
			20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46,
			199, 108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
		];

		assert_eq!(
			CurrencyId::try_from_symbol(
				"GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC:USDC".to_string()
			)
			.expect("Conversion should work"),
			CurrencyId::Stellar(Asset::AlphaNum4 { code: *b"USDC", issuer: issuer_bytes })
		);
		assert_eq!(
			CurrencyId::try_from_symbol(
				"gaknDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC:usdc".to_string()
			)
			.expect("Conversion should work"),
			CurrencyId::Stellar(Asset::AlphaNum4 { code: *b"USDC", issuer: issuer_bytes })
		);
	}

	#[test]
	fn test_try_from_symbol_works_for_xcm_currency() {
		assert_eq!(
			CurrencyId::try_from_symbol("0".to_string()).expect("Conversion should work"),
			CurrencyId::XCM(0)
		);
		assert_eq!(
			CurrencyId::try_from_symbol("1".to_string()).expect("Conversion should work"),
			CurrencyId::XCM(1)
		);
		assert_eq!(
			CurrencyId::try_from_symbol("255".to_string()).expect("Conversion should work"),
			CurrencyId::XCM(255)
		);
	}
}
