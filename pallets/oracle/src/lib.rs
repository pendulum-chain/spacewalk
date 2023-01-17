//! # Oracle Pallet
//! Based on the [specification](https://spec.interlay.io/spec/oracle.html).

// #![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
extern crate mocktopus;

use codec::{Decode, Encode, MaxEncodedLen};
// #[cfg(feature = "testing-utils")]
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	transactional,
};
#[cfg(test)]
use mocktopus::macros::mockable;
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{UniqueSaturatedInto, *},
	ArithmeticError, FixedPointNumber,
};
use sp_std::{convert::TryInto, vec::Vec};

use currency::Amount;
pub use default_weights::{SubstrateWeight, WeightInfo};
use orml_oracle::{DataProviderExtended, TimestampedValue};
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

#[cfg(test)]
#[cfg_attr(test, cfg(feature = "testing-utils"))]
pub mod mock;

pub mod types;

pub mod dia;
#[cfg(feature = "testing-utils")]
pub mod oracle_mock;
use orml_oracle::DataFeeder;

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
			TimestampedValue<Self::UnsignedFixedPoint, Self::Moment>,
		>;

		#[cfg(feature = "testing-utils")]
		type DataFeedProvider: orml_oracle::DataFeeder<
			OracleKey,
			TimestampedValue<Self::UnsignedFixedPoint, Self::Moment>,
			Self::AccountId,
		>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event emitted when exchange rate is set
		FeedValues {
			oracle_id: T::AccountId,
			values: Vec<(OracleKey, T::UnsignedFixedPoint)>,
		},
		AggregateUpdated {
			values: Vec<(OracleKey, Option<T::UnsignedFixedPoint>)>,
		},
		OracleKeysUpdated {
			oracle_keys: Vec<OracleKey>,
		},
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

	// Oracles keys required by runtime
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
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { max_delay: Default::default(), oracle_keys: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
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
		/// Feeds data from the oracles, e.g., the exchange rates. This function
		/// is intended to be API-compatible with orml-oracle.
		///
		/// # Arguments
		///
		/// * `values` - a vector of (key, value) pairs to submit
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::feed_values(values.len() as u32))]
		pub fn feed_values(
			origin: OriginFor<T>,
			values: Vec<(OracleKey, T::UnsignedFixedPoint)>,
		) -> DispatchResultWithPostInfo {
			// let signer = ensure_signed(origin)?;

			// fail if the signer is not an authorized oracle
			// ensure!(Self::is_authorized(&signer), Error::<T>::InvalidOracleSource);

			// Self::_feed_values(signer, values);
			Ok(Pays::No.into())
		}

		/// set oracle keys
		///
		/// # Arguments
		/// * `oracle_key` - list of oracle keys
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_authorized_oracle())]
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
			let is_outdated = current_time > price.timestamp + max_delay;
			if !is_outdated {
				updated_items.push((key.clone(), Some(price.value)));
			}
		}
		let updated_items_len = updated_items.len();
		if !updated_items.is_empty() {
			Self::deposit_event(Event::<T>::AggregateUpdated { values: updated_items });
		}

		let current_status_is_online = Self::is_oracle_online();
		let new_status_is_online = oracle_keys.len() > 0 && updated_items_len == oracle_keys.len();

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

		for (k, v) in values.clone() {
			let timestamped = TimestampedValue { timestamp: Self::get_current_time(), value: v };
			T::DataFeedProvider::feed_value(oracle.clone(), k.clone(), timestamped)
				.expect("Expect store value by key");
			if !oracle_keys.contains(&k) {
				oracle_keys.push(k);
			}
		}
		<OracleKeys<T>>::put(oracle_keys.clone());
		Self::deposit_event(Event::<T>::FeedValues { oracle_id: oracle, values });
		Ok(())
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
				// first convert to wrapped, then convert wrapped to the desired currency
				let base = Self::collateral_to_wrapped(amount.amount(), amount.currency())?; // maybe this does not work?
				Self::wrapped_to_collateral(base, currency_id)?
			},
		};
		Ok(Amount::new(converted, currency_id))
	}

	pub fn wrapped_to_collateral(
		amount: BalanceOf<T>,
		currency_id: CurrencyId,
	) -> Result<BalanceOf<T>, DispatchError> {
		let rate = Self::get_price(OracleKey::ExchangeRate(currency_id))?;
		let converted = rate.checked_mul_int(amount).ok_or(ArithmeticError::Overflow)?;
		Ok(converted)
	}

	pub fn collateral_to_wrapped(
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
		// Aggregate::<T>::insert(OracleKey::ExchangeRate(currency_id), exchange_rate);
		// this is useful for benchmark tests
		//TODO for testing get data from DataProvider as DataFeed trait
		Self::_feed_values(oracle, vec![((OracleKey::ExchangeRate(currency_id)), exchange_rate)]);
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
	) -> Option<TimestampedValue<T::UnsignedFixedPoint, T::Moment>> {
		T::DataProvider::get_no_op(key)
	}
}
