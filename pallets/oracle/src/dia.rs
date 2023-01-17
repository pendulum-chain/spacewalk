use dia_oracle::DiaOracle;
use orml_oracle::{DataProviderExtended, TimestampedValue};
pub use primitives::{oracle::Key as OracleKey, CurrencyId, TruncateFixedPointToInt};
use sp_std::marker;

use scale_info::prelude::string::String;
use sp_arithmetic::FixedU128;
use sp_runtime::traits::Convert;
use sp_std::{vec, vec::Vec};

const DOT_DIA_BLOCKCHAIN: &str = "Polkadot";
const DOT_DIA_SYMBOL: &str = "DOT";
const KSM_DIA_BLOCKCHAIN: &str = "Kusama";
const KSM_DIA_SYMBOL: &str = "KSM";
pub struct DiaOracleKeyConvertor;
impl Convert<OracleKey, Option<(Vec<u8>, Vec<u8>)>> for DiaOracleKeyConvertor {
	fn convert(spacwalk_oracle_key: OracleKey) -> Option<(Vec<u8>, Vec<u8>)> {
		match spacwalk_oracle_key {
			OracleKey::ExchangeRate(currency_id) => match currency_id {
				CurrencyId::Token(token_symbol) => match token_symbol {
					primitives::TokenSymbol::DOT =>
						return Some((
							DOT_DIA_BLOCKCHAIN.as_bytes().to_vec(),
							DOT_DIA_SYMBOL.as_bytes().to_vec(),
						)),
					primitives::TokenSymbol::KSM =>
						return Some((
							KSM_DIA_BLOCKCHAIN.as_bytes().to_vec(),
							KSM_DIA_SYMBOL.as_bytes().to_vec(),
						)),
					primitives::TokenSymbol::PEN => unimplemented!(),
					primitives::TokenSymbol::AMPE => unimplemented!(),
				},
				CurrencyId::ForeignAsset(_) => unimplemented!(),
				CurrencyId::Native => unimplemented!(),
				CurrencyId::StellarNative => unimplemented!(),
				CurrencyId::AlphaNum4 { .. } => unimplemented!(),
				CurrencyId::AlphaNum12 { .. } => unimplemented!(),
			},
			OracleKey::FeeEstimation => Some((vec![6u8], vec![])),
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
					return Some(OracleKey::ExchangeRate(CurrencyId::Token(
						primitives::TokenSymbol::DOT,
					)))
				} else if blockchain == KSM_DIA_BLOCKCHAIN && symbol == KSM_DIA_SYMBOL {
					return Some(OracleKey::ExchangeRate(CurrencyId::Token(
						primitives::TokenSymbol::KSM,
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
		let dia_key: Option<(Vec<u8>, Vec<u8>)> = ConvertKey::convert(key.clone());
		let Some((blockchain,symbol)) = dia_key else {
            return None;
        };

		let Ok(coin_info) = Dia::get_coin_info(blockchain, symbol) else {
            return None;
        };

		let Some(value) = ConvertPrice::convert(coin_info.price) else{
            return None;
        };
		let Some(timestamp) = ConvertMoment::convert(coin_info.last_update_timestamp) else{
            return None;
        };

		Some(TimestampedValue { value, timestamp })
	}

	///do not need this funtion implementation
	fn get_all_values() -> Vec<(OracleKey, Option<TimestampedValue<UnsignedFixedPoint, Moment>>)> {
		panic!("spacewalk oracle extension does not requre implementation of DataProviderExtended get_all_values function")
	}
}

pub struct MockOracleKeyConvertor;

impl Convert<OracleKey, Option<(Vec<u8>, Vec<u8>)>> for MockOracleKeyConvertor {
	fn convert(spacwalk_oracle_key: OracleKey) -> Option<(Vec<u8>, Vec<u8>)> {
		match spacwalk_oracle_key {
			OracleKey::ExchangeRate(currency_id) => match currency_id {
				CurrencyId::Token(token_symbol) => match token_symbol {
					primitives::TokenSymbol::DOT => return Some((vec![0u8], vec![1u8])),
					primitives::TokenSymbol::PEN => return Some((vec![0u8], vec![2u8])),
					primitives::TokenSymbol::KSM => return Some((vec![0u8], vec![3u8])),
					primitives::TokenSymbol::AMPE => return Some((vec![0u8], vec![4u8])),
				},
				CurrencyId::ForeignAsset(foreign_asset_id) =>
					Some((vec![1u8], foreign_asset_id.to_le_bytes().to_vec())),
				CurrencyId::Native => Some((vec![2u8], vec![])),
				CurrencyId::StellarNative => Some((vec![3u8], vec![])),
				CurrencyId::AlphaNum4 { code, .. } => Some((vec![4u8], code.to_vec())),
				CurrencyId::AlphaNum12 { code, .. } => Some((vec![5u8], code.to_vec())),
			},
			OracleKey::FeeEstimation => Some((vec![6u8], vec![])),
		}
	}
}

impl Convert<(Vec<u8>, Vec<u8>), Option<OracleKey>> for MockOracleKeyConvertor {
	fn convert(dia_oracle_key: (Vec<u8>, Vec<u8>)) -> Option<OracleKey> {
		let (blockchain, symbol) = dia_oracle_key;
		match blockchain[0] {
			0u8 => match symbol[0] {
				1 =>
					return Some(OracleKey::ExchangeRate(CurrencyId::Token(
						primitives::TokenSymbol::DOT,
					))),
				2 =>
					return Some(OracleKey::ExchangeRate(CurrencyId::Token(
						primitives::TokenSymbol::PEN,
					))),
				3 =>
					return Some(OracleKey::ExchangeRate(CurrencyId::Token(
						primitives::TokenSymbol::KSM,
					))),
				4 =>
					return Some(OracleKey::ExchangeRate(CurrencyId::Token(
						primitives::TokenSymbol::AMPE,
					))),
				_ => return None,
			},
			1u8 => {
				let x = [symbol[0], symbol[1], symbol[2], symbol[3]];
				let number = u32::from_le_bytes(x);
				Some(OracleKey::ExchangeRate(CurrencyId::ForeignAsset(number)))
			},
			2u8 => Some(OracleKey::ExchangeRate(CurrencyId::Native)),
			3u8 => Some(OracleKey::ExchangeRate(CurrencyId::StellarNative)),
			4u8 => {
				let vector = symbol;
				let code = [vector[0], vector[1], vector[2], vector[3]];
				Some(OracleKey::ExchangeRate(CurrencyId::AlphaNum4 { code, issuer: [0u8; 32] }))
			},
			5u8 => {
				let vector = symbol;
				let code = [
					vector[0], vector[1], vector[2], vector[3], vector[4], vector[5], vector[6],
					vector[7], vector[8], vector[9], vector[10], vector[11],
				];
				Some(OracleKey::ExchangeRate(CurrencyId::AlphaNum12 { code, issuer: [0u8; 32] }))
			},
			6u8 => Some(OracleKey::FeeEstimation),
			_ => None,
		}
	}
}
pub struct MockConvertPrice;
impl Convert<u128, Option<FixedU128>> for MockConvertPrice {
	fn convert(price: u128) -> Option<FixedU128> {
		Some(FixedU128::from_inner(price))
	}
}

pub struct MockConvertMoment;
impl Convert<u64, Option<u64>> for MockConvertMoment {
	fn convert(moment: u64) -> Option<u64> {
		Some(moment)
	}
}
