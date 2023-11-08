#[cfg(feature = "testing-utils")]
use sp_runtime::traits::Convert;
use sp_std::cmp::Ordering;

use sp_arithmetic::FixedU128;
pub type UnsignedFixedPoint = FixedU128;
use primitives::{oracle::Key, Asset, CurrencyId};
use sp_std::{vec, vec::Vec};

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct DataKey {
	pub blockchain: Vec<u8>,
	pub symbol: Vec<u8>,
}

impl PartialOrd<Self> for DataKey {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		self.blockchain.partial_cmp(&other.blockchain)
	}
}

impl Ord for DataKey {
	fn cmp(&self, other: &Self) -> Ordering {
		self.blockchain.cmp(&other.blockchain)
	}
}

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Data {
	pub key: DataKey,
	pub price: u128,
	pub timestamp: u64,
}

pub struct MockOracleKeyConvertor;

impl Convert<Key, Option<(Vec<u8>, Vec<u8>)>> for MockOracleKeyConvertor {
	fn convert(spacewalk_oracle_key: Key) -> Option<(Vec<u8>, Vec<u8>)> {
		match spacewalk_oracle_key {
			Key::ExchangeRate(currency_id) => match currency_id {
				CurrencyId::XCM(token_symbol) => Some((vec![0u8], vec![token_symbol])),
				CurrencyId::Native => Some((vec![2u8], vec![])),
				CurrencyId::StellarNative => Some((vec![3u8], vec![])),
				CurrencyId::Stellar(Asset::AlphaNum4 { code, .. }) =>
					Some((vec![4u8], code.to_vec())),
				CurrencyId::Stellar(Asset::AlphaNum12 { code, .. }) =>
					Some((vec![5u8], code.to_vec())),
				CurrencyId::ZenlinkLPToken(token1_id, token1_type, token2_id, token2_type) =>
					Some((vec![6], vec![token1_id, token1_type, token2_id, token2_type])),
				CurrencyId::Token(token_symbol) => {
					let token_symbol = token_symbol.to_be_bytes().to_vec();
					Some((vec![7], token_symbol))
				},
			},
		}
	}
}

impl Convert<(Vec<u8>, Vec<u8>), Option<Key>> for MockOracleKeyConvertor {
	fn convert(dia_oracle_key: (Vec<u8>, Vec<u8>)) -> Option<Key> {
		let (blockchain, symbol) = dia_oracle_key;
		match blockchain[0] {
			0u8 => Some(Key::ExchangeRate(CurrencyId::XCM(symbol[0]))),
			2u8 => Some(Key::ExchangeRate(CurrencyId::Native)),
			3u8 => Some(Key::ExchangeRate(CurrencyId::StellarNative)),
			4u8 => {
				let vector = symbol;
				let code = [vector[0], vector[1], vector[2], vector[3]];
				Some(Key::ExchangeRate(CurrencyId::AlphaNum4(code, [0u8; 32])))
			},
			5u8 => {
				let vector = symbol;
				let code = [
					vector[0], vector[1], vector[2], vector[3], vector[4], vector[5], vector[6],
					vector[7], vector[8], vector[9], vector[10], vector[11],
				];
				Some(Key::ExchangeRate(CurrencyId::AlphaNum12(code, [0u8; 32])))
			},
			6u8 => Some(Key::ExchangeRate(CurrencyId::ZenlinkLPToken(
				symbol[0], symbol[1], symbol[2], symbol[3],
			))),
			7u8 => {
				let symbol =
					u64::from_be_bytes(symbol.try_into().expect("Failed to convert to u64"));
				Some(Key::ExchangeRate(CurrencyId::Token(symbol)))
			},
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
