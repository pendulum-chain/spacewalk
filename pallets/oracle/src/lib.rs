//! # Oracle Pallet
//! Based on the [specification](https://spec.interlay.io/spec/oracle.html).

#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
extern crate mocktopus;

#[cfg(feature = "testing-utils")]
use frame_support::dispatch::DispatchResult;
use frame_support::{pallet_prelude::DispatchError, sp_runtime, transactional};
use frame_system::pallet_prelude::BlockNumberFor;

#[cfg(test)]
use mocktopus::macros::mockable;
use sp_runtime::{
	traits::{UniqueSaturatedInto, *},
	ArithmeticError, FixedPointNumber,
};
use sp_std::{convert::TryInto, vec::Vec};

use currency::Amount;
pub use default_weights::{SubstrateWeight, WeightInfo};
use orml_oracle::DataProviderExtended;
pub use pallet::*;
pub use primitives::{
	oracle::Key as OracleKey, CurrencyId, DecimalsLookup, TruncateFixedPointToInt,
};
use security::{ErrorCode, StatusCode};

use crate::types::{BalanceOf, UnsignedFixedPoint, Version};

mod ext;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod default_weights;

#[cfg(test)]
#[cfg_attr(test, cfg(feature = "testing-utils"))]
mod tests;

#[cfg(feature = "testing-utils")]
pub use dia_oracle::{CoinInfo, DiaOracle, PriceInfo};
#[cfg(feature = "testing-utils")]
pub use orml_oracle::{DataFeeder, DataProvider, TimestampedValue};
#[cfg(feature = "testing-utils")]
use spin::MutexGuard;

#[cfg(test)]
#[cfg_attr(test, cfg(feature = "testing-utils"))]
pub mod mock;

#[cfg(feature = "testing-utils")]
pub mod testing_utils;

pub mod types;

pub mod dia;

pub mod oracle_api;

pub use crate::oracle_api::*;

#[cfg(feature = "testing-utils")]
pub mod oracle_mock;

// We assume this value to be the decimals of our base currency (USD). It doesn't matter too much as
// long as it's consistent.
const USD_DECIMALS: u32 = 12;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_std::vec;

	use super::*;

	/// ## Configuration
	/// The pallet's configuration trait.
	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_timestamp::Config
		+ security::Config
		+ currency::Config<CurrencyId = CurrencyId>
	{
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		type DecimalsLookup: DecimalsLookup<CurrencyId = CurrencyId>;

		type DataProvider: DataProviderExtended<
			OracleKey,
			orml_oracle::TimestampedValue<Self::UnsignedFixedPoint, Self::Moment>,
		>;

		#[cfg(feature = "testing-utils")]
		type DataFeeder: testing_utils::DataFeederExtended<
			OracleKey,
			TimestampedValue<Self::UnsignedFixedPoint, Self::Moment>,
			Self::AccountId,
		>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		AggregateUpdated { values: Vec<(OracleKey, T::UnsignedFixedPoint)> },
		OracleKeysUpdated { oracle_keys: Vec<OracleKey> },
		MaxDelayUpdated { max_delay: T::Moment },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Not authorized to set exchange rate
		InvalidOracleSource,
		/// Exchange rate not specified or has expired
		MissingExchangeRate,
		/// Unable to convert value
		TryIntoIntError,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			Self::begin_block(n);
			<T as Config>::WeightInfo::on_initialize()
		}
	}

	/// Maximum delay (milliseconds) for a reported value to be used
	#[pallet::storage]
	#[pallet::getter(fn max_delay)]
	pub type MaxDelay<T: Config> = StorageValue<_, T::Moment, ValueQuery>;

	// Oracle keys indicating the available prices and used to retrieve them
	#[pallet::storage]
	#[pallet::getter(fn oracle_keys)]
	pub type OracleKeys<T: Config> = StorageValue<_, Vec<OracleKey>, ValueQuery>;

	#[pallet::type_value]
	pub(super) fn DefaultForStorageVersion() -> Version {
		Version::V0
	}

	/// Build storage at V1 (requires default 0).
	#[pallet::storage]
	#[pallet::getter(fn storage_version)]
	pub(super) type StorageVersion<T: Config> =
		StorageValue<_, Version, ValueQuery, DefaultForStorageVersion>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub max_delay: u32,
		pub oracle_keys: Vec<OracleKey>,
		#[serde(skip)]
		pub _phantom: sp_std::marker::PhantomData<T>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				max_delay: Default::default(),
				oracle_keys: vec![],
				_phantom: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			MaxDelay::<T>::put(T::Moment::from(self.max_delay));
			OracleKeys::<T>::put(self.oracle_keys.clone());
		}
	}

	#[pallet::pallet]
	#[pallet::without_storage_info] // MaxEncodedLen not implemented for vecs
	pub struct Pallet<T>(_);

	// The pallet's dispatchable functions.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// set oracle keys
		///
		/// # Arguments
		/// * `oracle_key` - list of oracle keys
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::update_oracle_keys())]
		#[transactional]
		pub fn update_oracle_keys(
			origin: OriginFor<T>,
			oracle_keys: Vec<OracleKey>,
		) -> DispatchResult {
			ensure_root(origin)?;
			<OracleKeys<T>>::put(oracle_keys.clone());
			Self::deposit_event(Event::OracleKeysUpdated { oracle_keys });
			Ok(())
		}

		/// Set the maximum delay (in milliseconds) for a reported value to be used
		///
		/// # Arguments
		/// * `new_max_delay` - new max delay in milliseconds
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::set_max_delay())]
		#[transactional]
		pub fn set_max_delay(origin: OriginFor<T>, new_max_delay: T::Moment) -> DispatchResult {
			ensure_root(origin)?;
			<MaxDelay<T>>::put(new_max_delay);

			Self::deposit_event(Event::MaxDelayUpdated { max_delay: new_max_delay });
			Ok(())
		}
	}
}

#[cfg_attr(test, mockable)]
impl<T: Config> Pallet<T> {
	// the function is public only for testing purposes. function should be use only by this pallet
	// inside on_initialize hook
	pub fn begin_block(_height: BlockNumberFor<T>) {
		let oracle_keys: Vec<_> = OracleKeys::<T>::get();

		let current_time = Self::get_current_time();

		let mut updated_items = Vec::new();
		let max_delay = Self::get_max_delay();
		for key in oracle_keys.iter() {
			let price = Self::get_timestamped(key);
			let Some(price) = price else { continue };
			let is_outdated = current_time > price.timestamp + max_delay;
			if !is_outdated {
				updated_items.push((key.clone(), price.value));
			}
		}
		let updated_items_len = updated_items.len();
		if !updated_items.is_empty() {
			Self::deposit_event(Event::<T>::AggregateUpdated { values: updated_items });
		}

		let current_status_is_online = Self::is_oracle_online();
		let new_status_is_online = !oracle_keys.is_empty() &&
			updated_items_len > 0 &&
			updated_items_len == oracle_keys.len();

		if current_status_is_online != new_status_is_online {
			if new_status_is_online {
				Self::recover_from_oracle_offline();
			} else {
				Self::report_oracle_offline();
			}
		}
	}

	// public only for testing purposes
	#[cfg(feature = "testing-utils")]
	pub fn feed_values(
		oracle: T::AccountId,
		values: Vec<(OracleKey, T::UnsignedFixedPoint)>,
	) -> DispatchResult {
		frame_support::ensure!(
			!values.is_empty(),
			"The provided vector of fed values cannot be empty."
		);

		let mut oracle_keys: Vec<_> = <OracleKeys<T>>::get();

		for (key, value) in values {
			let timestamped =
				orml_oracle::TimestampedValue { timestamp: Self::get_current_time(), value };
			T::DataFeeder::feed_value(Some(oracle.clone()), key.clone(), timestamped)
				.expect("Expect store value by key");
			if !oracle_keys.contains(&key) {
				oracle_keys.push(key);
			}
		}
		<OracleKeys<T>>::put(oracle_keys.clone());
		Ok(())
	}

	// public only for testing purposes
	#[cfg(feature = "testing-utils")]
	pub fn clear_values() -> DispatchResult {
		use crate::testing_utils::DataFeederExtended;
		T::DataFeeder::clear_all_values()
	}

	// public only for testing purposes
	#[cfg(feature = "testing-utils")]
	pub fn acquire_lock() -> MutexGuard<'static, ()> {
		use crate::testing_utils::DataFeederExtended;
		T::DataFeeder::acquire_lock()
	}

	/// Public getters

	/// Get the exchange rate in planck per satoshi
	pub fn get_price(key: OracleKey) -> Result<UnsignedFixedPoint<T>, DispatchError> {
		ext::security::ensure_parachain_status_running::<T>()?;

		let Some(price) = T::DataProvider::get_no_op(&key) else {
			return Err(Error::<T>::MissingExchangeRate.into());
		};
		Ok(price.value)
	}

	pub fn convert(
		amount: &Amount<T>,
		currency_id: T::CurrencyId,
	) -> Result<Amount<T>, DispatchError> {
		let converted = match (amount.currency(), currency_id) {
			(x, y) if x == y => amount.amount(),
			(_, _) => Self::convert_amount(
				amount.amount(),
				Self::get_price(OracleKey::ExchangeRate(amount.currency()))?,
				Self::get_price(OracleKey::ExchangeRate(currency_id))?,
				T::DecimalsLookup::decimals(amount.currency()),
				T::DecimalsLookup::decimals(currency_id),
			)?,
		};
		Ok(Amount::new(converted, currency_id))
	}

	pub fn currency_to_usd(
		amount: BalanceOf<T>,
		currency_id: CurrencyId,
	) -> Result<BalanceOf<T>, DispatchError> {
		// Rate from asset to USD
		let asset_rate = Self::get_price(OracleKey::ExchangeRate(currency_id))?;
		// Rate from USD to USD is 1
		let usd_rate = UnsignedFixedPoint::<T>::one();

		Self::convert_amount(
			amount,
			asset_rate,
			usd_rate,
			T::DecimalsLookup::decimals(currency_id),
			USD_DECIMALS,
		)
	}

	pub fn usd_to_currency(
		amount: BalanceOf<T>,
		currency_id: CurrencyId,
	) -> Result<BalanceOf<T>, DispatchError> {
		// Rate from asset to USD
		let asset_rate = Self::get_price(OracleKey::ExchangeRate(currency_id))?;
		// Rate from USD to USD is 1
		let usd_rate = UnsignedFixedPoint::<T>::one();

		Self::convert_amount(
			amount,
			usd_rate,
			asset_rate,
			USD_DECIMALS,
			T::DecimalsLookup::decimals(currency_id),
		)
	}

	fn convert_amount(
		from_amount: BalanceOf<T>,
		from_price: T::UnsignedFixedPoint,
		to_price: T::UnsignedFixedPoint,
		from_decimals: u32,
		to_decimals: u32,
	) -> Result<BalanceOf<T>, DispatchError> {
		if from_amount.is_zero() {
			return Ok(Zero::zero());
		}

		let from_amount = T::UnsignedFixedPoint::from_inner(from_amount);

		if from_decimals > to_decimals {
			// result = from_amount * from_price / to_price / 10^(from_decimals - to_decimals)
			let to_amount = from_price
				.checked_mul(&from_amount)
				.ok_or(ArithmeticError::Overflow)?
				.checked_div(&to_price)
				.ok_or(ArithmeticError::Underflow)?
				.checked_div(
					&UnsignedFixedPoint::<T>::checked_from_integer(
						10u128.pow(from_decimals.saturating_sub(to_decimals)),
					)
					.ok_or(Error::<T>::TryIntoIntError)?,
				)
				.ok_or(ArithmeticError::Underflow)?
				.into_inner()
				.unique_saturated_into();

			Ok(to_amount)
		} else {
			// result = from_amount * from_price * 10^(to_decimals - from_decimals) / to_price
			let to_amount = from_price
				.checked_mul(&from_amount)
				.ok_or(ArithmeticError::Overflow)?
				.checked_mul(
					&UnsignedFixedPoint::<T>::checked_from_integer(
						10u128.pow(to_decimals.saturating_sub(from_decimals)),
					)
					.ok_or(Error::<T>::TryIntoIntError)?,
				)
				.ok_or(ArithmeticError::Overflow)?
				.checked_div(&to_price)
				.ok_or(ArithmeticError::Underflow)?
				.into_inner()
				.unique_saturated_into();

			Ok(to_amount)
		}
	}

	pub fn get_exchange_rate(
		currency_id: CurrencyId,
	) -> Result<T::UnsignedFixedPoint, DispatchError> {
		let rate = Self::get_price(OracleKey::ExchangeRate(currency_id))?;
		Ok(rate)
	}

	/// Private getters and setters
	fn get_max_delay() -> T::Moment {
		<MaxDelay<T>>::get()
	}

	/// Set the current exchange rate.
	///
	/// # Arguments
	///
	/// * `exchange_rate` - i.e. planck per satoshi
	#[cfg(feature = "testing-utils")]
	pub fn _set_exchange_rate(
		oracle: T::AccountId,
		currency_id: CurrencyId,
		exchange_rate: UnsignedFixedPoint<T>,
	) -> DispatchResult {
		use sp_std::vec;
		frame_support::assert_ok!(Self::feed_values(
			oracle,
			vec![((OracleKey::ExchangeRate(currency_id)), exchange_rate)]
		));
		Self::recover_from_oracle_offline();
		Ok(())
	}

	fn is_oracle_online() -> bool {
		!ext::security::get_errors::<T>().contains(&ErrorCode::OracleOffline)
	}

	fn report_oracle_offline() {
		ext::security::set_status::<T>(StatusCode::Error);
		ext::security::insert_error::<T>(ErrorCode::OracleOffline);
	}

	fn recover_from_oracle_offline() {
		ext::security::recover_from_oracle_offline::<T>()
	}

	/// Returns the current timestamp
	fn get_current_time() -> T::Moment {
		<pallet_timestamp::Pallet<T>>::get()
	}

	fn get_timestamped(
		key: &OracleKey,
	) -> Option<orml_oracle::TimestampedValue<T::UnsignedFixedPoint, T::Moment>> {
		T::DataProvider::get_no_op(key)
	}
}
