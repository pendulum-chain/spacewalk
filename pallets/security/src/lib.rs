//! # Security Module
//! Based on the [specification](https://spec.interlay.io/spec/security.html).

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
extern crate mocktopus;
#[cfg(test)]
use mocktopus::macros::mockable;

use codec::Encode;
use frame_support::{dispatch::DispatchResult, sp_runtime, transactional};
use frame_system::pallet_prelude::BlockNumberFor;
use sha2::{Digest, Sha256};
use sp_core::{H256, U256};
use sp_runtime::{traits::*, ArithmeticError, DispatchError};
use sp_std::{collections::btree_set::BTreeSet, convert::TryInto, prelude::*, vec};

pub use pallet::*;

#[doc(inline)]
pub use crate::types::{ErrorCode, StatusCode};

pub mod types;
pub use default_weights::{SubstrateWeight, WeightInfo};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod default_weights;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use super::*;

	/// ## Configuration
	/// The pallet's configuration trait.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		RecoverFromErrors { new_status: StatusCode, cleared_errors: Vec<ErrorCode> },
		UpdateActiveBlock { block_number: BlockNumberFor<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Parachain is not running.
		ParachainNotRunning,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			Self::increment_active_block();
			<T as Config>::WeightInfo::on_initialize()
		}
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub initial_status: StatusCode,
		#[serde(skip)]
		pub _phantom: sp_std::marker::PhantomData<T>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { initial_status: StatusCode::Error, _phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::set_status(self.initial_status);

			Pallet::<T>::insert_error(ErrorCode::OracleOffline);
		}
	}

	/// Integer/Enum defining the current state of the Spacewalk-Parachain.
	#[pallet::storage]
	#[pallet::getter(fn parachain_status)]
	pub type ParachainStatus<T: Config> = StorageValue<_, StatusCode, ValueQuery>;

	/// Set of ErrorCodes, indicating the reason for an "Error" ParachainStatus.
	#[pallet::storage]
	#[pallet::getter(fn errors)]
	pub type Errors<T: Config> = StorageValue<_, BTreeSet<ErrorCode>, ValueQuery>;

	/// Integer increment-only counter, used to prevent collisions when generating identifiers
	/// for e.g. issue, redeem or replace requests.
	#[pallet::storage]
	pub type Nonce<T: Config> = StorageValue<_, U256, ValueQuery>;

	/// Like frame_system::block_number, but this one only increments if the parachain status is
	/// RUNNING. This variable is used to keep track of durations, such as the issue/redeem/replace
	/// expiry. If the parachain is not RUNNING, no payment proofs can be submitted, and it wouldn't
	/// be fair to punish the user/vault. By using this variable we ensure that they have sufficient
	/// time to submit their proof.
	#[pallet::storage]
	#[pallet::getter(fn active_block_number)]
	pub type ActiveBlockCount<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	// The pallet's dispatchable functions.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the parachain status code.
		///
		/// # Arguments
		///
		/// * `origin` - the dispatch origin of this call (must be _Root_)
		/// * `status_code` - the status code to set
		///
		/// # Weight: `O(1)`
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_parachain_status())]
		#[transactional]
		pub fn set_parachain_status(
			origin: OriginFor<T>,
			status_code: StatusCode,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			Self::set_status(status_code);
			Ok(().into())
		}

		/// Insert a new parachain error.
		///
		/// # Arguments
		///
		/// * `origin` - the dispatch origin of this call (must be _Root_)
		/// * `error_code` - the error code to insert
		///
		/// # Weight: `O(1)`
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::insert_parachain_error())]
		#[transactional]
		pub fn insert_parachain_error(
			origin: OriginFor<T>,
			error_code: ErrorCode,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			Self::insert_error(error_code);
			Ok(().into())
		}

		/// Remove a parachain error.
		///
		/// # Arguments
		///
		/// * `origin` - the dispatch origin of this call (must be _Root_)
		/// * `error_code` - the error code to remove
		///
		/// # Weight: `O(1)`
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_parachain_error())]
		#[transactional]
		pub fn remove_parachain_error(
			origin: OriginFor<T>,
			error_code: ErrorCode,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			Self::remove_error(error_code);
			Ok(().into())
		}
	}
}

// "Internal" functions, callable by code.
#[allow(
	clippy::forget_non_drop,
	clippy::swap_ptr_to_ref,
	forgetting_references,
	forgetting_copy_types
)]
#[cfg_attr(test, mockable)]
impl<T: Config> Pallet<T> {
	/// Ensures the Parachain is RUNNING
	pub fn ensure_parachain_status_running() -> DispatchResult {
		if <ParachainStatus<T>>::get() == StatusCode::Running {
			Ok(())
		} else {
			Err(Error::<T>::ParachainNotRunning.into())
		}
	}

	/// Checks if the Parachain has Shutdown
	pub fn is_parachain_shutdown() -> bool {
		Self::parachain_status() == StatusCode::Shutdown
	}

	/// Checks if the Parachain has a OracleOffline Error state
	pub fn is_parachain_error_oracle_offline() -> bool {
		Self::parachain_status() == StatusCode::Error &&
			<Errors<T>>::get().contains(&ErrorCode::OracleOffline)
	}

	/// Sets the given `StatusCode`.
	///
	/// # Arguments
	///
	/// * `status_code` - to set in storage.
	pub fn set_status(status_code: StatusCode) {
		<ParachainStatus<T>>::set(status_code);
	}

	/// Get the current set of `ErrorCode`.
	pub fn get_errors() -> BTreeSet<ErrorCode> {
		<Errors<T>>::get()
	}

	/// Inserts the given `ErrorCode`.
	///
	/// # Arguments
	///
	/// * `error_code` - the error to insert.
	pub fn insert_error(error_code: ErrorCode) {
		<Errors<T>>::mutate(|errors| {
			errors.insert(error_code);
		})
	}

	/// Removes the given `ErrorCode`.
	///
	/// # Arguments
	///
	/// * `error_code` - the error to remove.
	pub fn remove_error(error_code: ErrorCode) {
		<Errors<T>>::mutate(|errors| {
			errors.remove(&error_code);
		})
	}

	pub fn parachain_block_expired(
		opentime: BlockNumberFor<T>,
		period: BlockNumberFor<T>,
	) -> Result<bool, DispatchError> {
		let expiration_block = opentime.checked_add(&period).ok_or(ArithmeticError::Overflow)?;
		Ok(Self::active_block_number() > expiration_block)
	}

	fn recover_from_(error_codes: Vec<ErrorCode>) {
		for error_code in error_codes.clone() {
			Self::remove_error(error_code);
		}

		if Self::get_errors().is_empty() {
			Self::set_status(StatusCode::Running);
		}

		Self::deposit_event(Event::RecoverFromErrors {
			new_status: Self::parachain_status(),
			cleared_errors: error_codes,
		});
	}

	/// Recovers the Spacewalk Parachain state from an `ORACLE_OFFLINE` error
	/// and sets ParachainStatus to `RUNNING` if there are no other errors.
	pub fn recover_from_oracle_offline() {
		Self::recover_from_(vec![ErrorCode::OracleOffline])
	}

	/// Increment and return the `Nonce`.
	fn get_nonce() -> U256 {
		<Nonce<T>>::mutate(|n| {
			let (res, _) = (*n).overflowing_add(U256::one());
			*n = res;
			*n
		})
	}

	fn increment_active_block() {
		if Self::parachain_status() == StatusCode::Running {
			let height = <ActiveBlockCount<T>>::mutate(|n| {
				*n = n.saturating_add(1u32.into());
				*n
			});
			Self::deposit_event(Event::UpdateActiveBlock { block_number: height });
		}
	}

	/// Generates a 256-bit unique hash from an `AccountId` and the
	/// internal (auto-incrementing) `Nonce` to prevent replay attacks.
	///
	/// # Arguments
	///
	/// * `id`: Parachain account identifier.
	pub fn get_secure_id() -> H256 {
		let mut hasher = Sha256::default();
		hasher.update(frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default().encode());
		hasher.update(Self::get_nonce().encode());
		// supplement with prev block hash to prevent replays
		// even if the `Nonce` is reset (i.e. purge-chain)
		hasher.update(frame_system::Pallet::<T>::parent_hash());
		let mut result = [0; 32];
		result.copy_from_slice(&hasher.finalize()[..]);
		H256(result)
	}

	/// for testing purposes only!
	#[cfg(feature = "testing-utils")]
	pub fn set_active_block_number(n: BlockNumberFor<T>) {
		ActiveBlockCount::<T>::set(n);
	}
}
