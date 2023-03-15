use std::convert::TryFrom;

use crate::{CurrencyId, Error};

/// Convert a ticker symbol into a `CurrencyId` at runtime
pub trait TryFromSymbol: Sized {
	fn try_from_symbol(symbol: String) -> Result<Self, Error>;
}

impl TryFromSymbol for CurrencyId {
	fn try_from_symbol(symbol: String) -> Result<Self, Error> {
		let uppercase_symbol = symbol.to_uppercase();

		// Try to build stellar asset
		let parts = uppercase_symbol.split(':').collect::<Vec<&str>>();
		if parts.len() == 2 {
			let issuer = parts[0].trim();
			let code = parts[1].trim();
			return CurrencyId::try_from((code, issuer)).map_err(|_| Error::InvalidCurrency)
		}

		// We assume that it is an XCM currency so we try to parse it as a number
		if let Ok(id) = uppercase_symbol.parse::<u8>() {
			return Ok(CurrencyId::XCM(id))
		} else {
			return Err(Error::InvalidCurrency)
		}
	}
}
