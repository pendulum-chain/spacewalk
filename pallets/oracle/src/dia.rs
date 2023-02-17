use dia_oracle::DiaOracle;
use orml_oracle::{DataProviderExtended, TimestampedValue};
pub use primitives::{oracle::Key as OracleKey, CurrencyId, TruncateFixedPointToInt};
use sp_std::marker;

use scale_info::prelude::string::String;
use sp_runtime::traits::Convert;
use sp_std::vec::Vec;

const DOT_DIA_BLOCKCHAIN: &str = "Polkadot";
const DOT_DIA_SYMBOL: &str = "DOT";
const KSM_DIA_BLOCKCHAIN: &str = "Kusama";
const KSM_DIA_SYMBOL: &str = "KSM";
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
		base.to_ascii_uppercase().to_vec()
	};

	[base_currency, "-".as_bytes().to_vec(), TARGET_QUOTE.as_bytes().to_vec()].concat()
}

pub struct DiaOracleKeyConvertor;
impl Convert<OracleKey, Option<(Vec<u8>, Vec<u8>)>> for DiaOracleKeyConvertor {
	fn convert(spacewalk_oracle_key: OracleKey) -> Option<(Vec<u8>, Vec<u8>)> {
		match spacewalk_oracle_key {
			OracleKey::ExchangeRate(currency_id) => match currency_id {
				CurrencyId::XCM(token_symbol) => match token_symbol {
					primitives::ForeignCurrencyId::DOT =>
						return Some((
							DOT_DIA_BLOCKCHAIN.as_bytes().to_vec(),
							DOT_DIA_SYMBOL.as_bytes().to_vec(),
						)),
					primitives::ForeignCurrencyId::KSM =>
						return Some((
							KSM_DIA_BLOCKCHAIN.as_bytes().to_vec(),
							KSM_DIA_SYMBOL.as_bytes().to_vec(),
						)),
					_ => unimplemented!(),
				},
				CurrencyId::Native => unimplemented!(),
				CurrencyId::StellarNative => Some((
					STELLAR_DIA_BLOCKCHAIN.as_bytes().to_vec(),
					STELLAR_DIA_SYMBOL.as_bytes().to_vec(),
				)),
				CurrencyId::Stellar(primitives::Asset::AlphaNum4 { code, .. }) => {
					let fiat_quote = construct_fiat_usd_symbol_for_currency(code.to_vec());

					Some((FIAT_DIA_BLOCKCHAIN.as_bytes().to_vec(), fiat_quote))
				},
				CurrencyId::Stellar(primitives::Asset::AlphaNum12 { .. }) => unimplemented!(),
			},
		}
	}
}

impl Convert<(Vec<u8>, Vec<u8>), Option<OracleKey>> for DiaOracleKeyConvertor {
	fn convert(dia_oracle_key: (Vec<u8>, Vec<u8>)) -> Option<OracleKey> {
		let (blockchain, symbol) = dia_oracle_key;
		let blockchain = String::from_utf8(blockchain);
		let symbol = String::from_utf8(symbol);
		return match (blockchain, symbol) {
			(Ok(blockchain), Ok(symbol)) => {
				if blockchain == DOT_DIA_BLOCKCHAIN && symbol == DOT_DIA_SYMBOL {
					Some(OracleKey::ExchangeRate(CurrencyId::XCM(
						primitives::ForeignCurrencyId::DOT,
					)))
				} else if blockchain == KSM_DIA_BLOCKCHAIN && symbol == KSM_DIA_SYMBOL {
					Some(OracleKey::ExchangeRate(CurrencyId::XCM(
						primitives::ForeignCurrencyId::KSM,
					)))
				} else if blockchain == FIAT_DIA_BLOCKCHAIN {
					Some(OracleKey::ExchangeRate(CurrencyId::Stellar(
						primitives::Asset::AlphaNum4 {
							code: symbol.as_bytes().try_into().unwrap(),
							issuer: Default::default(),
						},
					)))
				} else {
					None
				}
			},
			(_, _) => None,
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

		let Ok(coin_info) = Dia::get_coin_info(blockchain, symbol) else {
            return None;
        };

		let value = ConvertPrice::convert(coin_info.price)?;
		let Some(timestamp) = ConvertMoment::convert(coin_info.last_update_timestamp) else{
            return None;
        };

		Some(TimestampedValue { value, timestamp })
	}

	/// We do not need the implementation of this function
	fn get_all_values() -> Vec<(OracleKey, Option<TimestampedValue<UnsignedFixedPoint, Moment>>)> {
		panic!("The Spacewalk oracle extension does not require the implementation of the DataProviderExtended::get_all_values() function")
	}
}
