use sp_runtime::traits::Convert;
use sp_std::{cmp::Ordering, sync::Arc};

use sp_arithmetic::FixedU128;
pub type UnsignedFixedPoint = FixedU128;
use primitives::{oracle::Key, Asset, CurrencyId};
use sp_std::{collections::btree_map::BTreeMap, vec, vec::Vec};
use spin::RwLock;

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

pub struct MockConvertMoment<Moment>(sp_std::marker::PhantomData<Moment>);
impl<Moment> Convert<Moment, Option<Moment>> for MockConvertMoment<Moment> {
	fn convert(moment: Moment) -> Option<Moment> {
		Some(moment)
	}
}

// Implementation of re-usable mock DiaOracle

lazy_static::lazy_static! {
	static ref COINS: Arc<RwLock<BTreeMap<DataKey, Data>>> = Arc::new(RwLock::new(BTreeMap::<DataKey, Data>::new()));
}

pub struct MockDiaOracle;
impl dia_oracle::DiaOracle for MockDiaOracle {
	fn get_coin_info(
		blockchain: Vec<u8>,
		symbol: Vec<u8>,
	) -> Result<dia_oracle::CoinInfo, sp_runtime::DispatchError> {
		let key = (blockchain, symbol);
		let data_key = DataKey { blockchain: key.0.clone(), symbol: key.1.clone() };
		let mut result: Option<Data> = None;

		let coins = COINS.read();
		let o = coins.get(&data_key);
		match o {
			Some(i) => result = Some(i.clone()),
			None => {},
		};
		let Some(result) = result else {
			return Err(sp_runtime::DispatchError::Other(""));
		};
		let mut coin_info = dia_oracle::CoinInfo::default();
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

use orml_oracle::{DataFeeder, DataProvider, TimestampedValue};

pub struct MockDataCollector<AccountId, Moment>(
	sp_std::marker::PhantomData<AccountId>,
	sp_std::marker::PhantomData<Moment>,
);

//DataFeeder required to implement DataProvider trait but there no need to implement get function
impl<AccountId, Moment> DataProvider<Key, TimestampedValue<UnsignedFixedPoint, Moment>>
	for MockDataCollector<AccountId, Moment>
{
	fn get(_key: &Key) -> Option<TimestampedValue<UnsignedFixedPoint, Moment>> {
		unimplemented!("Not required to implement DataProvider get function")
	}
}
impl<AccountId, Moment: Into<u64>>
	DataFeeder<Key, TimestampedValue<UnsignedFixedPoint, Moment>, AccountId>
	for MockDataCollector<AccountId, Moment>
{
	fn feed_value(
		_who: AccountId,
		key: Key,
		value: TimestampedValue<UnsignedFixedPoint, Moment>,
	) -> sp_runtime::DispatchResult {
		let key = MockOracleKeyConvertor::convert(key).unwrap();
		let r = value.value.into_inner();

		let data_key = DataKey { blockchain: key.0.clone(), symbol: key.1.clone() };
		let data = Data { key: data_key.clone(), price: r, timestamp: value.timestamp.into() };

		let mut coins = COINS.write();
		coins.insert(data_key, data);

		Ok(())
	}
}
