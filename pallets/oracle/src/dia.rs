use dia_oracle::DiaOracle;
use orml_oracle::{DataProviderExtended, TimestampedValue};
pub use primitives::{oracle::Key as OracleKey, CurrencyId, TruncateFixedPointToInt, remove_trailing_zeroes};
use sp_std::marker;

use sp_runtime::traits::Convert;
use sp_std::vec::Vec;
use crate::log;

const STELLAR_DIA_BLOCKCHAIN: &str = "Stellar";
const STELLAR_DIA_SYMBOL: &str = "XLM";
const FIAT_DIA_BLOCKCHAIN: &str = "FIAT";

// We always want to quote against USD
const TARGET_QUOTE: &str = "USD";

// This constructs a fiat quote symbol for a given base currency
fn construct_fiat_usd_symbol_for_currency(base: Vec<u8>) -> Vec<u8> {
	// We need to convert USDC to USD
	// In the future we might have to do this for other currencies as well
	let base_currency = if base.to_ascii_uppercase().to_vec() == "USDC".as_bytes().to_vec() {
		"USD".as_bytes().to_vec()
	} else {
		// Ensure we use uppercase
		 remove_trailing_zeroes(&base.to_ascii_uppercase()).to_vec()
	};

	[base_currency, "-".as_bytes().to_vec(), TARGET_QUOTE.as_bytes().to_vec()].concat()
}

/// A trait to define the blockchain name and symbol of the native currency.
/// This is required by the DiaOracleKeyConvertor to be able to derive the key used
/// for the price information of the native currency.
pub trait NativeCurrencyKey {
	/// define the token symbol
	fn native_symbol() -> Vec<u8>;
	// define the blockchain name
	fn native_chain() -> Vec<u8>;
}

pub trait XCMCurrencyConversion {
	/// Converts a XCM token symbol to a DiaOracle key. The result is of form (blockchain, symbol).
	fn convert_to_dia_currency_id(token_symbol: u8) -> Option<(Vec<u8>, Vec<u8>)>;
	/// Converts a DiaOracle key to a XCM token symbol.
	fn convert_from_dia_currency_id(blockchain: Vec<u8>, symbol: Vec<u8>) -> Option<u8>;
}

pub struct DiaOracleKeyConvertor<T: NativeCurrencyKey + XCMCurrencyConversion>(
	marker::PhantomData<T>,
);

impl<T: NativeCurrencyKey + XCMCurrencyConversion> Convert<OracleKey, Option<(Vec<u8>, Vec<u8>)>>
	for DiaOracleKeyConvertor<T>
{
	fn convert(spacewalk_oracle_key: OracleKey) -> Option<(Vec<u8>, Vec<u8>)> {
		match spacewalk_oracle_key {
			OracleKey::ExchangeRate(currency_id) => match currency_id {
				CurrencyId::XCM(token_symbol) => T::convert_to_dia_currency_id(token_symbol),
				CurrencyId::Native => Some((T::native_chain(), T::native_symbol())),
				CurrencyId::StellarNative => Some((
					STELLAR_DIA_BLOCKCHAIN.as_bytes().to_vec(),
					STELLAR_DIA_SYMBOL.as_bytes().to_vec(),
				)),
				CurrencyId::Stellar(primitives::Asset::AlphaNum4 { code, .. }) => {
					let fiat_quote = construct_fiat_usd_symbol_for_currency(code.to_vec());

					log::info!("WHAT DA FAAAAAAACXKKKK ORACLE DIA: {fiat_quote:?}");
					Some((FIAT_DIA_BLOCKCHAIN.as_bytes().to_vec(), fiat_quote))
				},
				CurrencyId::Stellar(primitives::Asset::AlphaNum12 { .. }) => unimplemented!(),
			},
		}
	}
}

impl<T: NativeCurrencyKey + XCMCurrencyConversion> Convert<(Vec<u8>, Vec<u8>), Option<OracleKey>>
	for DiaOracleKeyConvertor<T>
{
	fn convert(dia_oracle_key: (Vec<u8>, Vec<u8>)) -> Option<OracleKey> {
		let (blockchain, symbol) = dia_oracle_key;
		let xcm_currency_id = T::convert_from_dia_currency_id(blockchain.clone(), symbol.clone());

		if let Some(xcm_currency_id) = xcm_currency_id {
			Some(OracleKey::ExchangeRate(CurrencyId::XCM(xcm_currency_id)))
		} else if blockchain == T::native_chain() && symbol == T::native_symbol() {
			Some(OracleKey::ExchangeRate(CurrencyId::Native))
		} else if blockchain == STELLAR_DIA_BLOCKCHAIN.as_bytes().to_vec() &&
			symbol == STELLAR_DIA_SYMBOL.as_bytes().to_vec()
		{
			Some(OracleKey::ExchangeRate(CurrencyId::StellarNative))
		} else if blockchain == FIAT_DIA_BLOCKCHAIN.as_bytes().to_vec() {
			Some(OracleKey::ExchangeRate(CurrencyId::Stellar(primitives::Asset::AlphaNum4 {
				code: symbol.try_into().unwrap(),
				issuer: Default::default(),
			})))
		} else {
			None
		}
	}
}

pub struct DiaOracleAdapter<
	DiaPallet: DiaOracle,
	UnsignedFixedPoint,
	Moment,
	ConvertKey,
	ConvertPrice,
	ConvertMoment,
>(
	marker::PhantomData<(
		DiaPallet,
		UnsignedFixedPoint,
		Moment,
		ConvertKey,
		ConvertPrice,
		ConvertMoment,
	)>,
);

impl<Dia, UnsignedFixedPoint, Moment, ConvertKey, ConvertPrice, ConvertMoment>
	DataProviderExtended<OracleKey, TimestampedValue<UnsignedFixedPoint, Moment>>
	for DiaOracleAdapter<Dia, UnsignedFixedPoint, Moment, ConvertKey, ConvertPrice, ConvertMoment>
where
	Dia: DiaOracle,
	ConvertKey: Convert<OracleKey, Option<(Vec<u8>, Vec<u8>)>>
		+ Convert<(Vec<u8>, Vec<u8>), Option<OracleKey>>,
	ConvertPrice: Convert<u128, Option<UnsignedFixedPoint>>,
	ConvertMoment: Convert<u64, Option<Moment>>,
{
	fn get_no_op(key: &OracleKey) -> Option<TimestampedValue<UnsignedFixedPoint, Moment>> {
		let (blockchain, symbol) = ConvertKey::convert(key.clone())?;
		log::info!("WHAT DA FAAAAAAACXKKKK DIA: symbol: {symbol:?} of chain: {blockchain:?}");
		let Ok(coin_info) = Dia::get_coin_info(blockchain, symbol) else {
			log::info!("WHAT DA FAAAAAAACXKKKK DIA: NO COIN INFO ");
            return None;
        };

		let value = ConvertPrice::convert(coin_info.price)?;
		log::info!("WHAT DA FAAAAAAACXKKKK DIA: converted price value succeeded");

		let Some(timestamp) = ConvertMoment::convert(coin_info.last_update_timestamp) else{
			log::info!("WHAT DA FAAAAAAACXKKKK DIA: NO TIMESTAMP FOUND FOR {coin_info:?}");
			return None;
        };

		log::info!("WHAT DA FAAAAAAACXKKKK DIA: return!!!!!");
		Some(TimestampedValue { value, timestamp })
	}

	/// We do not need the implementation of this function
	fn get_all_values() -> Vec<(OracleKey, Option<TimestampedValue<UnsignedFixedPoint, Moment>>)> {
		panic!("The Spacewalk oracle extension does not require the implementation of the DataProviderExtended::get_all_values() function")
	}
}
