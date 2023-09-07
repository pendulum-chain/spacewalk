//! # Oracle Pallet
//! Based on the [specification](https://spec.interlay.io/spec/oracle.html).

#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
extern crate mocktopus;

#[cfg(feature = "testing-utils")]
use frame_support::dispatch::DispatchResult;
use frame_support::{dispatch::DispatchError, transactional};
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
pub use primitives::{oracle::Key as OracleKey, CurrencyId, TruncateFixedPointToInt};
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

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

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

		type DataProvider: DataProviderExtended<
			OracleKey,
			orml_oracle::TimestampedValue<Self::UnsignedFixedPoint, Self::Moment>,
		>;

		#[cfg(feature = "testing-utils")]
		type DataFeedProvider: testing_utils::DataFeederExtended<
			OracleKey,
			TimestampedValue<Self::UnsignedFixedPoint, Self::Moment>,
			Self::AccountId,
		>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
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
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
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

	#[derive(Default)]
	#[pallet::genesis_config]
	pub struct GenesisConfig {
		pub max_delay: u32,
		pub oracle_keys: Vec<OracleKey>,
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
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
	pub fn begin_block(_height: T::BlockNumber) {
		let oracle_keys: Vec<_> = OracleKeys::<T>::get();

		let current_time = Self::get_current_time();

		let mut updated_items = Vec::new();
		let max_delay = Self::get_max_delay();
		for key in oracle_keys.iter() {
			let price = Self::get_timestamped(key);
			let Some(price) = price else{
				continue;
			};
			// Here, no outdated values are found although one price should be expired in the test
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
		let new_status_is_online = oracle_keys.len() > 0 &&
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
	pub fn _feed_values(
		oracle: T::AccountId,
		values: Vec<(OracleKey, T::UnsignedFixedPoint)>,
	) -> DispatchResult {
		let mut oracle_keys: Vec<_> = <OracleKeys<T>>::get();

		for (key, value) in values {
			let timestamped =
				orml_oracle::TimestampedValue { timestamp: Self::get_current_time(), value };
			T::DataFeedProvider::feed_value(oracle.clone(), key.clone(), timestamped)
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
	pub fn _clear_values() -> DispatchResult {
		use crate::testing_utils::DataFeederExtended;
		T::DataFeedProvider::clear_all_values()
	}

	// public only for testing purposes
	#[cfg(feature = "testing-utils")]
	pub fn _acquire_lock() -> MutexGuard<'static, ()> {
		use crate::testing_utils::DataFeederExtended;
		T::DataFeedProvider::acquire_lock()
	}

	/// Public getters

	/// Get the exchange rate in planck per satoshi
	pub fn get_price(key: OracleKey) -> Result<UnsignedFixedPoint<T>, DispatchError> {
		ext::security::ensure_parachain_status_running::<T>()?;

		let Some(price) = T::DataProvider::get_no_op(&key) else{
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
			(_, _) => {
				// First convert to USD, then convert USD to the desired currency
				let base = Self::currency_to_usd(amount.amount(), amount.currency())?;
				Self::usd_to_currency(base, currency_id)?
			},
		};
		Ok(Amount::new(converted, currency_id))
	}

	pub fn currency_to_usd(
		amount: BalanceOf<T>,
		currency_id: CurrencyId,
	) -> Result<BalanceOf<T>, DispatchError> {
		let rate = Self::get_price(OracleKey::ExchangeRate(currency_id))?;
		let converted = rate.checked_mul_int(amount).ok_or(ArithmeticError::Overflow)?;
		Ok(converted)
	}

	pub fn usd_to_currency(
		amount: BalanceOf<T>,
		currency_id: CurrencyId,
	) -> Result<BalanceOf<T>, DispatchError> {
		let rate = Self::get_price(OracleKey::ExchangeRate(currency_id))?;
		if amount.is_zero() {
			return Ok(Zero::zero())
		}

		// The code below performs `amount/rate`, plus necessary type conversions
		Ok(T::UnsignedFixedPoint::checked_from_integer(amount)
			.ok_or(Error::<T>::TryIntoIntError)?
			.checked_div(&rate)
			.ok_or(ArithmeticError::Underflow)?
			.truncate_to_inner()
			.ok_or(Error::<T>::TryIntoIntError)?
			.unique_saturated_into())
	}

	/// Private getters and setters
	fn get_max_delay() -> T::Moment {
		<MaxDelay<T>>::get()
	}

	/// TODO
	/// Set the current exchange rate. ONLY FOR TESTING.
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
		frame_support::assert_ok!(Self::_feed_values(
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
