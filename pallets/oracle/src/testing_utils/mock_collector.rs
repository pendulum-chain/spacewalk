use once_cell::race::OnceBox;
use orml_oracle::{DataFeeder, DataProvider, TimestampedValue};
use sp_arithmetic::FixedU128;
use sp_runtime::{traits::Convert, DispatchResult};
use sp_std::{boxed::Box, collections::btree_map::BTreeMap, vec, vec::Vec};
use spin::{Mutex, MutexGuard, RwLock};

use primitives::oracle::Key;

use crate::testing_utils::mock_convertors::MockOracleKeyConvertor;

type UnsignedFixedPoint = FixedU128;
type MapKey = u128;

// This is a global variable that stores the mock data.
// A `OnceBox` is used to ensure that the mock data is only initialized once.
static COINS: OnceBox<RwLock<BTreeMap<MapKey, Data>>> = OnceBox::new();
// This lock can be used to synchronize access to the DIA mock. It is used to prevent race
// conditions when running tests in parallel.
static LOCK: OnceBox<Mutex<()>> = OnceBox::new();

// Extends the orml_oracle::DataFeeder trait with other functions
pub trait DataFeederExtended<Key, Value, AccountId>: DataFeeder<Key, Value, AccountId> {
	fn clear_all_values() -> DispatchResult;
	/// This function is used to acquire a lock on the static `LOCK` variable. This can be used to
	/// prevent race conditions when running tests in parallel.
	fn acquire_lock() -> MutexGuard<'static, ()>;
}

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

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Data {
	pub key: MapKey,
	pub price: u128,
	pub timestamp: u64,
}

/// This struct is used to mock the DIA oracle. It implements the `DiaOracle` trait and uses the
/// static `COINS` variable to store the mock data.
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

	fn get_value(
		_blockchain: Vec<u8>,
		_symbol: Vec<u8>,
	) -> Result<dia_oracle::PriceInfo, sp_runtime::DispatchError> {
		// We don't need to implement this function for the mock
		unimplemented!(
			"DiaOracleAdapter implementation of DataProviderExtended does not use this function."
		)
	}
}

/// This struct is used to make it possible to feed values to the mock oracle. It implements the
/// `DataFeederExtended` trait and uses the static `COINS` variable to store the mock data.
pub struct MockDataFeeder<AccountId, Moment>(
	sp_std::marker::PhantomData<AccountId>,
	sp_std::marker::PhantomData<Moment>,
);

impl<AccountId, Moment> DataProvider<Key, TimestampedValue<UnsignedFixedPoint, Moment>>
	for MockDataFeeder<AccountId, Moment>
{
	// We need to implement the DataFeeder trait to the MockDataFeeder but this function is never
	// used
	fn get(_key: &Key) -> Option<TimestampedValue<UnsignedFixedPoint, Moment>> {
		unimplemented!("Not required to implement DataProvider get function")
	}
}

impl<AccountId, Moment: Into<u64>>
	DataFeeder<Key, TimestampedValue<UnsignedFixedPoint, Moment>, AccountId>
	for MockDataFeeder<AccountId, Moment>
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
	for MockDataFeeder<AccountId, Moment>
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
