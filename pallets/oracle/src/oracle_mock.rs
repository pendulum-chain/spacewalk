use sp_runtime::traits::Convert;

use once_cell::race::OnceBox;
use primitives::{oracle::Key, Asset, CurrencyId};
use sp_arithmetic::FixedU128;
use sp_std::{boxed::Box, collections::btree_map::BTreeMap, vec, vec::Vec};
use spin::{Mutex, MutexGuard, RwLock};

use orml_oracle::{DataFeeder, DataProvider, TimestampedValue};
use sp_runtime::DispatchResult;

// Extends the orml_oracle::DataFeeder trait with a clear_all_values function.
pub trait DataFeederExtended<Key, Value, AccountId>: DataFeeder<Key, Value, AccountId> {
	fn clear_all_values() -> sp_runtime::DispatchResult;
	fn acquire_lock() -> MutexGuard<'static, ()>;
}

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Data {
	pub key: u128,
	pub price: u128,
	pub timestamp: u64,
}

pub type UnsignedFixedPoint = FixedU128;
type MapKey = u128;

// A function to uniquely derive a key from blockchain and symbol
fn derive_key(blockchain: Vec<u8>, symbol: Vec<u8>) -> MapKey {
	// Set symbol and blockchain to 0 if it is not provided
	let symbol = if symbol.is_empty() { vec![0u8; 32] } else { symbol };
	let blockchain = if blockchain.is_empty() { vec![0u8; 32] } else { blockchain };

	// Sum up the blockchain and symbol vectors
	let blockchain_sum: u128 = blockchain.iter().fold(0u128, |acc, x| acc + (*x as u128));
	let symbol_sum: u128 = symbol.iter().fold(0u128, |acc, x| acc + (*x as u128));

	// Use bitshift operation to combine blockchain and symbol into a single u128 key
	let key: u128 = (blockchain_sum) << 64 | symbol_sum;
	key
}

pub struct MockOracleKeyConvertor;

impl Convert<Key, Option<(Vec<u8>, Vec<u8>)>> for MockOracleKeyConvertor {
	fn convert(spacewalk_oracle_key: Key) -> Option<(Vec<u8>, Vec<u8>)> {
		match spacewalk_oracle_key {
			Key::ExchangeRate(currency_id) => match currency_id {
				CurrencyId::XCM(token_symbol) => Some((vec![0u8], vec![token_symbol])),
				CurrencyId::Native => Some((vec![2u8], vec![0])),
				CurrencyId::StellarNative => Some((vec![3u8], vec![0])),
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

static COINS: OnceBox<RwLock<BTreeMap<MapKey, Data>>> = OnceBox::new();
// This lock can be used to synchronize access to the DIA mock. It is used to prevent race
// conditions when running tests in parallel.
static LOCK: OnceBox<Mutex<()>> = OnceBox::new();

pub struct MockDiaOracle;
impl dia_oracle::DiaOracle for MockDiaOracle {
	fn get_coin_info(
		blockchain: Vec<u8>,
		symbol: Vec<u8>,
	) -> Result<dia_oracle::CoinInfo, sp_runtime::DispatchError> {
		let key = derive_key(blockchain, symbol);

		let coins = COINS
			.get_or_init(|| Box::new(RwLock::new(BTreeMap::<MapKey, Data>::new())))
			.read();
		let coin_data = coins.get(&key);
		let Some(result) = coin_data else {
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
pub struct MockDataCollector<AccountId, Moment>(
	sp_std::marker::PhantomData<AccountId>,
	sp_std::marker::PhantomData<Moment>,
);

impl<AccountId, Moment> DataProvider<Key, TimestampedValue<UnsignedFixedPoint, Moment>>
	for MockDataCollector<AccountId, Moment>
{
	// We need to implement the DataFeeder trait to the MockDataCollector but this function is never
	// used
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
	) -> DispatchResult {
		let (blockchain, symbol) = MockOracleKeyConvertor::convert(key).unwrap();
		let price = value.value.into_inner();

		let key = derive_key(blockchain, symbol);
		let data = Data { key, price, timestamp: value.timestamp.into().clone() };

		let mut coins = COINS
			.get_or_init(|| Box::new(RwLock::new(BTreeMap::<MapKey, Data>::new())))
			.write();
		coins.insert(key, data);

		Ok(())
	}
}

impl<AccountId, Moment: Into<u64>>
	DataFeederExtended<Key, TimestampedValue<UnsignedFixedPoint, Moment>, AccountId>
	for MockDataCollector<AccountId, Moment>
{
	fn clear_all_values() -> DispatchResult {
		let mut coins = COINS
			.get_or_init(|| Box::new(RwLock::new(BTreeMap::<MapKey, Data>::new())))
			.write();
		coins.clear();
		Ok(())
	}

	fn acquire_lock() -> MutexGuard<'static, ()> {
		let mutex = LOCK.get_or_init(|| Box::new(Mutex::new(())));
		mutex.lock()
	}
}
