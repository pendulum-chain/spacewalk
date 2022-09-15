#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod traits;
mod types;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use crate::{
		traits::{ExternalizedMessage, Validator},
		types::TransactionEnvelopeXdr,
	};

	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		// The maximum amount of validators stored on-chain
		#[pallet::constant]
		type ValidatorLimit: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		ValidatorLimitExceeded,
	}

	#[pallet::storage]
	#[pallet::getter(fn validators)]
	pub type Validators<T: Config> =
		StorageValue<_, BoundedVec<Validator, T::ValidatorLimit>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub validators: Vec<Validator>,
		phantom: PhantomData<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { validators: vec![], phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let vec = BoundedVec::<Validator, T::ValidatorLimit>::try_from(self.validators.clone());
			assert!(vec.is_ok());
			Validators::<T>::put(vec.unwrap());
		}
	}

	// Extrinsics
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// This extrinsic is used to update/replace the current set of validators.
		#[pallet::weight(10_000)]
		pub fn update_tier_1_validator_set(
			origin: OriginFor<T>,
			validators: Vec<Validator>,
		) -> DispatchResult {
			// Limit this call to root
			let _ = ensure_root(origin)?;

			let vec = BoundedVec::<Validator, T::ValidatorLimit>::try_from(validators)
				.map_err(|_| Error::<T>::ValidatorLimitExceeded)?;
			Validators::<T>::put(vec);

			Ok(())
		}
	}

	// Helper functions
	impl<T: Config> Pallet<T> {
		/// This function is used to verify if a give transaction was executed on the Stellar
		/// network.
		pub fn validate_stellar_transaction(
			transaction_envelope_xdr: TransactionEnvelopeXdr,
			externalized_messages: Vec<ExternalizedMessage>,
			transaction_set: Vec<Vec<TransactionEnvelopeXdr>>,
		) -> DispatchResult {
			Ok(())
		}
	}
}
