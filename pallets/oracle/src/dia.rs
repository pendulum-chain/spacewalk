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
pub struct DiaOracleKeyConvertor;
impl Convert<OracleKey, Option<(Vec<u8>, Vec<u8>)>> for DiaOracleKeyConvertor {
	fn convert(spacwalk_oracle_key: OracleKey) -> Option<(Vec<u8>, Vec<u8>)> {
		match spacwalk_oracle_key {
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
				CurrencyId::StellarNative => unimplemented!(),
				CurrencyId::AlphaNum4 { .. } => unimplemented!(),
				CurrencyId::AlphaNum12 { .. } => unimplemented!(),
			},
		}
	}
}

impl Convert<(Vec<u8>, Vec<u8>), Option<OracleKey>> for DiaOracleKeyConvertor {
	fn convert(dia_oracle_key: (Vec<u8>, Vec<u8>)) -> Option<OracleKey> {
		let (blockchain, symbol) = dia_oracle_key;
		let blockchain = String::from_utf8(blockchain);
		let symbol = String::from_utf8(symbol);
		match (blockchain, symbol) {
			(Ok(blockchain), Ok(symbol)) => {
				if blockchain == DOT_DIA_BLOCKCHAIN && symbol == DOT_DIA_SYMBOL {
					return Some(OracleKey::ExchangeRate(CurrencyId::XCM(
						primitives::ForeignCurrencyId::DOT,
					)))
				} else if blockchain == KSM_DIA_BLOCKCHAIN && symbol == KSM_DIA_SYMBOL {
					return Some(OracleKey::ExchangeRate(CurrencyId::XCM(
						primitives::ForeignCurrencyId::KSM,
					)))
				} else {
					return None
				}
			},
			(_, _) => return None,
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
