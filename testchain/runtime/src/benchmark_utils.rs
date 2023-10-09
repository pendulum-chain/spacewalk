use dia_oracle::{CoinInfo, DiaOracle};
use orml_oracle::TimestampedValue;
use orml_traits::DataProvider;
use sp_runtime::traits::Convert;
use sp_std::{collections::btree_map::BTreeMap as HashMap, vec::Vec};
use spin::mutex::Mutex;

pub use oracle::oracle_mock::{MockConvertMoment, MockConvertPrice, MockOracleKeyConvertor};
use oracle::{oracle_mock::*, OracleKey};

use crate::{AccountId, Moment};

lazy_static::lazy_static! {
	static ref COINS: Mutex<HashMap<DataKey, Data>> = Mutex::new(HashMap::<DataKey, Data>::new());
}

pub struct MockDiaOracle;
impl DiaOracle for MockDiaOracle {
	fn get_coin_info(
		blockchain: Vec<u8>,
		symbol: Vec<u8>,
	) -> Result<CoinInfo, sp_runtime::DispatchError> {
		let key = (blockchain, symbol);
		let data_key = DataKey { blockchain: key.0.clone(), symbol: key.1 };
		let mut result: Option<Data> = None;
		let map = COINS.lock();
		let data_option = map.get(&data_key);
		match data_option {
			Some(data) => result = Some(data.clone()),
			None => {},
		};

		let Some(result) = result else {
			return Err(sp_runtime::DispatchError::Other(""))
		};
		let mut coin_info = CoinInfo::default();
		coin_info.price = result.price;
		coin_info.last_update_timestamp = result.timestamp;

		Ok(coin_info)
	}

	//Spacewalk DiaOracleAdapter does not use get_value function. There is no need to implement
	// this function.
	fn get_value(
		_blockchain: Vec<u8>,
		_symbol: Vec<u8>,
	) -> Result<dia_oracle::PriceInfo, sp_runtime::DispatchError> {
		unimplemented!(
			"DiaOracleAdapter implementation of DataProviderExtended does not use this function."
		)
	}
}

// Integration tests feed_value prices data directly to DIA oracle pallet. Need defatult
// implementation only for compile this runtime for integration tests inside 'runtime' package.
pub struct DataCollector;

//DataFeeder required to implement DataProvider trait but there no need to implement get function
impl DataProvider<OracleKey, TimestampedValue<UnsignedFixedPoint, Moment>> for DataCollector {
	fn get(_key: &OracleKey) -> Option<TimestampedValue<UnsignedFixedPoint, Moment>> {
		unimplemented!("Not required to implement DataProvider get function")
	}
}

impl orml_oracle::DataFeeder<OracleKey, TimestampedValue<UnsignedFixedPoint, Moment>, AccountId>
	for DataCollector
{
	fn feed_value(
		_who: AccountId,
		key: OracleKey,
		value: TimestampedValue<UnsignedFixedPoint, Moment>,
	) -> sp_runtime::DispatchResult {
		let key = MockOracleKeyConvertor::convert(key).unwrap();
		let price = value.value.into_inner();

		let data_key = DataKey { blockchain: key.0.clone(), symbol: key.1 };
		let data = Data { key: data_key.clone(), price, timestamp: value.timestamp };

		COINS.lock().insert(data_key, data);
		Ok(())
	}
}
