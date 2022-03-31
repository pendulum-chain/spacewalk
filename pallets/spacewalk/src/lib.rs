#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;
use frame_support::sp_std::{prelude::*, str};
use frame_support::sp_runtime::traits::{Convert, StaticLookup};
use substrate_stellar_sdk as stellar;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use orml_traits::MultiCurrency;
type BalanceOf<T> =
    <<T as Config>::Currency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;

type CurrencyIdOf<T> =
    <<T as Config>::Currency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;
	
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::error::LookupError;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: MultiCurrency<Self::AccountId>;
        type AddressConversion: StaticLookup<Source = Self::AccountId, Target = stellar::PublicKey>;
        type StringCurrencyConversion: Convert<(Vec<u8>, Vec<u8>), Result<CurrencyIdOf<Self>, ()>>;
		
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// User initiated a redeem. [CurrencyIdOf<T>, T::AccountId, BalanceOf<T>]
		InitiateRedeem(CurrencyIdOf<T>, T::AccountId, BalanceOf<T>),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
        // Failed to change a balance
        BalanceChangeError,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {

        #[pallet::weight(100000)]
        pub fn initate_redeem(
            origin: OriginFor<T>,
            asset_code: Vec<u8>,
            asset_issuer: Vec<u8>,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
			let currency_id = T::StringCurrencyConversion::convert((asset_code, asset_issuer))
                .map_err(|_| LookupError)?;
            let pendulum_account_id = ensure_signed(origin)?;
            //let stellar_address = T::AddressConversion::lookup(pendulum_account_id.clone())?;

            T::Currency::withdraw(currency_id.clone(), &pendulum_account_id, amount)
                .map_err(|_| <Error<T>>::BalanceChangeError)?;

            Self::deposit_event(Event::InitiateRedeem(currency_id, pendulum_account_id, amount));
            Ok(().into())
        }
	}
}
