use std::{
	collections::BTreeMap,
	convert::TryFrom,
	sync::{Mutex, MutexGuard},
};

use primitives::{CurrencyId::Token, CurrencyInfo, TokenSymbol::*};

use crate::{types::*, CurrencyId, Error};

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
			return CurrencyId::try_from((code, issuer)).map_err(|err| Error::InvalidCurrency)
		}

		// try hardcoded currencies first
		match uppercase_symbol.as_str() {
			id if id == DOT.symbol() => Ok(Token(DOT)),
			id if id == PEN.symbol() => Ok(Token(PEN)),
			id if id == KSM.symbol() => Ok(Token(KSM)),
			id if id == AMPE.symbol() => Ok(Token(AMPE)),
			_ => Err(Error::InvalidCurrency),
		}
	}
}
