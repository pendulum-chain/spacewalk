#[cfg(feature = "testing-utils")]
use std::{cell::RefCell, sync::RwLock};

use dia_oracle::{CoinInfo, DiaOracle};
use orml_oracle::{DataProvider, TimestampedValue};
use sp_runtime::traits::Convert;

use sp_arithmetic::{FixedI128, FixedU128};
pub type UnsignedFixedPoint = FixedU128;
use primitives::{oracle::Key, CurrencyId};
type Moment = u64;

#[derive(Clone)]
struct Data {
	pub key: (Vec<u8>, Vec<u8>),
	pub price: u128,
	pub timestamp: u64,
}

thread_local! {
	static COINS: RefCell<Vec<Data>> = RefCell::new(vec![]);
}

pub type AccountId = u64;

pub struct MockDiaOracle;
impl DiaOracle for MockDiaOracle {
	fn get_coin_info(
		blockchain: Vec<u8>,
		symbol: Vec<u8>,
	) -> Result<dia_oracle::CoinInfo, sp_runtime::DispatchError> {
		let key = (blockchain, symbol);
		let mut result: Option<Data> = None;
		COINS.with(|c| {
			let r = c.borrow();
			for i in &*r {
				if i.key == key.clone() {
					result = Some(i.clone());
					break
				}
			}
		});
		let Some(result) = result else {
			return Err(sp_runtime::DispatchError::Other(""));
		};
		let mut coin_info = CoinInfo::default();
		coin_info.price = result.price;
		coin_info.last_update_timestamp = result.timestamp;

		Ok(coin_info)
	}

	fn get_value(
		blockchain: Vec<u8>,
		symbol: Vec<u8>,
	) -> Result<dia_oracle::PriceInfo, sp_runtime::DispatchError> {
		todo!()
	}
}

pub struct DataCollector;
impl DataProvider<Key, TimestampedValue<UnsignedFixedPoint, Moment>> for DataCollector {
	fn get(key: &Key) -> Option<TimestampedValue<UnsignedFixedPoint, Moment>> {
		todo!()
	}
}
impl orml_oracle::DataFeeder<Key, TimestampedValue<UnsignedFixedPoint, Moment>, AccountId>
	for DataCollector
{
	fn feed_value(
		who: AccountId,
		key: Key,
		value: TimestampedValue<UnsignedFixedPoint, Moment>,
	) -> sp_runtime::DispatchResult {
		let key = MockOracleKeyConvertor::convert(key).unwrap();
		let r = value.value.into_inner();

		let data = Data { key, price: r, timestamp: value.timestamp };

		COINS.with(|coins| {
			let mut r = coins.borrow_mut();
			r.push(data)
		});
		Ok(())
	}
}

pub struct MockOracleKeyConvertor;

impl Convert<Key, Option<(Vec<u8>, Vec<u8>)>> for MockOracleKeyConvertor {
	fn convert(spacwalk_oracle_key: Key) -> Option<(Vec<u8>, Vec<u8>)> {
		match spacwalk_oracle_key {
			Key::ExchangeRate(currency_id) => match currency_id {
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
			Key::FeeEstimation => Some((vec![6u8], vec![])),
		}
	}
}

impl Convert<(Vec<u8>, Vec<u8>), Option<Key>> for MockOracleKeyConvertor {
	fn convert(dia_oracle_key: (Vec<u8>, Vec<u8>)) -> Option<Key> {
		let (blockchain, symbol) = dia_oracle_key;
		match blockchain[0] {
			0u8 => match symbol[0] {
				1 =>
					return Some(Key::ExchangeRate(CurrencyId::Token(primitives::TokenSymbol::DOT))),
				2 =>
					return Some(Key::ExchangeRate(CurrencyId::Token(primitives::TokenSymbol::PEN))),
				3 =>
					return Some(Key::ExchangeRate(CurrencyId::Token(primitives::TokenSymbol::KSM))),
				4 =>
					return Some(Key::ExchangeRate(CurrencyId::Token(primitives::TokenSymbol::AMPE))),
				_ => return None,
			},
			1u8 => {
				let x = [symbol[0], symbol[1], symbol[2], symbol[3]];
				let number = u32::from_le_bytes(x);
				Some(Key::ExchangeRate(CurrencyId::ForeignAsset(number)))
			},
			2u8 => Some(Key::ExchangeRate(CurrencyId::Native)),
			3u8 => Some(Key::ExchangeRate(CurrencyId::StellarNative)),
			4u8 => {
				let vector = symbol;
				let code = [vector[0], vector[1], vector[2], vector[3]];
				Some(Key::ExchangeRate(CurrencyId::AlphaNum4 { code, issuer: [0u8; 32] }))
			},
			5u8 => {
				let vector = symbol;
				let code = [
					vector[0], vector[1], vector[2], vector[3], vector[4], vector[5], vector[6],
					vector[7], vector[8], vector[9], vector[10], vector[11],
				];
				Some(Key::ExchangeRate(CurrencyId::AlphaNum12 { code, issuer: [0u8; 32] }))
			},
			6u8 => Some(Key::FeeEstimation),
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
