#[cfg(feature = "testing-utils")]
use std::{cell::RefCell, sync::RwLock};

use dia_oracle::{CoinInfo, DiaOracle};
use orml_oracle::{DataProvider, TimestampedValue};
use sp_runtime::traits::Convert;

use crate::dia::MockDiaOracleConvertor;
use sp_arithmetic::{FixedI128, FixedU128};
pub type UnsignedFixedPoint = FixedU128;
use primitives::oracle::Key;
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
		let key = MockDiaOracleConvertor::convert(key).unwrap();
		let r = value.value.into_inner();

		let data = Data { key, price: r, timestamp: value.timestamp };

		COINS.with(|coins| {
			let mut r = coins.borrow_mut();
			r.push(data)
		});
		Ok(())
	}
}
