use crate::currency::CurrencyId;
use frame_support::error::LookupError;
use sp_runtime::traits::{Convert, StaticLookup};
use sp_std::{convert::TryInto, str::from_utf8, vec::Vec};
use substrate_stellar_sdk::{Asset, PublicKey};

pub struct CurrencyConversion;

fn to_look_up_error(_: &'static str) -> LookupError {
	LookupError
}

impl StaticLookup for CurrencyConversion {
	type Source = CurrencyId;
	type Target = Asset;

	fn lookup(
		currency_id: <Self as StaticLookup>::Source,
	) -> Result<<Self as StaticLookup>::Target, LookupError> {
		let asset_conversion_result: Result<Asset, &str> = currency_id.try_into();
		asset_conversion_result.map_err(to_look_up_error)
	}

	fn unlookup(stellar_asset: <Self as StaticLookup>::Target) -> <Self as StaticLookup>::Source {
		CurrencyId::from(stellar_asset)
	}
}

pub struct StringCurrencyConversion;

impl Convert<(Vec<u8>, Vec<u8>), Result<CurrencyId, ()>> for StringCurrencyConversion {
	fn convert(a: (Vec<u8>, Vec<u8>)) -> Result<CurrencyId, ()> {
		let public_key = PublicKey::from_encoding(a.1).map_err(|_| ())?;
		let asset_code = from_utf8(a.0.as_slice()).map_err(|_| ())?;
		(asset_code, public_key.into_binary()).try_into().map_err(|_| ())
	}
}

#[cfg(test)]
mod tests {
    use super::*;


	#[test]
	fn test_currency_conversion() {

		let currency_id = CurrencyId::Native;

		let currency_lookup = CurrencyConversion::lookup(currency_id);
		assert!(currency_lookup.is_ok());

		let currency_lookup = currency_lookup?;
		assert_eq!(currency_id, Asset::AssetTypeNative);

		let lookup_orig = BalanceConversion::unlookup(currency_lookup);
		assert_eq!(lookup_orig, currency_id);
	}
}