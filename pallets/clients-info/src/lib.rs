//! # ClientsInfo Module
//! Stores information about clients used for the network.

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

mod default_weights;

pub use default_weights::{SubstrateWeight, WeightInfo};

use frame_support::{dispatch::DispatchResult, traits::Get, transactional, BoundedVec};

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type NameOf<T> = BoundedVec<u8, <T as pallet::Config>::MaxNameLength>;
type UriOf<T> = BoundedVec<u8, <T as pallet::Config>::MaxUriLength>;

#[derive(Encode, Decode, Eq, PartialEq, Clone, Default, TypeInfo, Debug, MaxEncodedLen)]
pub struct ClientRelease<Uri, Hash> {
	/// URI to the client release binary.
	pub uri: Uri,
	/// The SHA256 checksum of the client binary.
	pub checksum: Hash,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use crate::*;

	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>>
			+ Into<<Self as frame_system::Config>::RuntimeEvent>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// The maximum length of a client name.
		#[pallet::constant]
		type MaxNameLength: Get<u32>;

		/// The maximum length of a client URI.
		#[pallet::constant]
		type MaxUriLength: Get<u32>;
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			crate::upgrade_client_releases::try_upgrade_current_client_releases::<T>()
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the current client release version, in case of a bug fix or patch.
		///
		/// # Arguments
		/// * `client_name` - raw byte string representation of the client name (e.g. `b"vault"`)
		/// * `release` - The release information for the given `client_name`
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_current_client_release(T::MaxNameLength::get(), T::MaxUriLength::get()))]
		#[transactional]
		pub fn set_current_client_release(
			origin: OriginFor<T>,
			client_name: NameOf<T>,
			release: ClientRelease<UriOf<T>, T::Hash>,
		) -> DispatchResult {
			Pallet::<T>::check_origin_rights(origin)?;
			CurrentClientReleases::<T>::insert(client_name, release.clone());
			Self::deposit_event(Event::<T>::ApplyClientRelease { release });
			Ok(())
		}

		/// Sets the pending client release version. To be batched alongside the
		/// `parachainSystem.authorizeUpgrade` Cumulus call.
		/// Clients include the vault, oracle, and faucet.
		///
		/// # Arguments
		/// * `client_name` - raw byte string representation of the client name (e.g. `b"vault"`)
		/// * `release` - The release information for the given `client_name`
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_pending_client_release(T::MaxNameLength::get(), T::MaxUriLength::get()))]
		#[transactional]
		pub fn set_pending_client_release(
			origin: OriginFor<T>,
			client_name: NameOf<T>,
			release: ClientRelease<UriOf<T>, T::Hash>,
		) -> DispatchResult {
			Pallet::<T>::check_origin_rights(origin)?;
			PendingClientReleases::<T>::insert(client_name, release.clone());
			Self::deposit_event(Event::<T>::NotifyClientRelease { release });
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::authorize_account())]
		#[transactional]
		pub fn authorize_account(origin: OriginFor<T>, account_id: T::AccountId) -> DispatchResult {
			Pallet::<T>::check_origin_rights(origin)?;

			if !<AuthorizedAccounts<T>>::contains_key(&account_id) {
				Self::deposit_event(Event::<T>::AccountIdAuthorized(account_id.clone()));
				<AuthorizedAccounts<T>>::insert(account_id, ());
			}

			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::deauthorize_account())]
		#[transactional]
		pub fn deauthorize_account(
			origin: OriginFor<T>,
			account_id: T::AccountId,
		) -> DispatchResult {
			if ensure_root(origin.clone()).is_err() {
				let origin_account_id = Pallet::<T>::check_non_root_rights(origin)?;
				ensure!(
					account_id != origin_account_id,
					Error::<T>::UserUnableToDeauthorizeThemself
				);
			}

			if <AuthorizedAccounts<T>>::contains_key(&account_id) {
				Self::deposit_event(Event::<T>::AccountIdDeauthorized(account_id.clone()));
				<AuthorizedAccounts<T>>::remove(account_id);
			}

			Ok(())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NotifyClientRelease { release: ClientRelease<UriOf<T>, T::Hash> },
		ApplyClientRelease { release: ClientRelease<UriOf<T>, T::Hash> },
		AccountIdAuthorized(T::AccountId),
		AccountIdDeauthorized(T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		ThisAccountIdIsNotAuthorized,
		UserUnableToDeauthorizeThemself,
	}

	/// Mapping of client name (string literal represented as bytes) to its release details.
	#[pallet::storage]
	#[pallet::getter(fn current_client_release)]
	pub(super) type CurrentClientReleases<T: Config> =
		StorageMap<_, Blake2_128Concat, NameOf<T>, ClientRelease<UriOf<T>, T::Hash>, OptionQuery>;

	/// Mapping of client name (string literal represented as bytes) to its pending release details.
	#[pallet::storage]
	#[pallet::getter(fn pending_client_release)]
	pub(super) type PendingClientReleases<T: Config> =
		StorageMap<_, Blake2_128Concat, NameOf<T>, ClientRelease<UriOf<T>, T::Hash>, OptionQuery>;

	/// List of all authorized accounts
	#[pallet::storage]
	#[pallet::getter(fn authorized_accounts)]
	pub(super) type AuthorizedAccounts<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, ()>;

	impl<T: Config> Pallet<T> {
		fn check_non_root_rights(origin: OriginFor<T>) -> Result<T::AccountId, DispatchError> {
			let origin_account_id = ensure_signed(origin)?;

			ensure!(
				<AuthorizedAccounts<T>>::contains_key(&origin_account_id),
				Error::<T>::ThisAccountIdIsNotAuthorized
			);

			Ok(origin_account_id)
		}

		fn check_origin_rights(origin: OriginFor<T>) -> DispatchResult {
			if ensure_root(origin.clone()).is_err() {
				Pallet::<T>::check_non_root_rights(origin)?;
			}

			Ok(())
		}
	}
}

pub mod upgrade_client_releases {

	use crate::*;
	use frame_support::weights::Weight;

	/// For each pending client release, set the current release to that.
	/// The pending release entry is removed.
	pub fn try_upgrade_current_client_releases<T: Config>() -> Weight {
		let mut reads = 0;
		for (key, release) in PendingClientReleases::<T>::drain() {
			//log::info!("Upgrading client release for key {:?}", key);
			CurrentClientReleases::<T>::insert(key, release.clone());
			Pallet::<T>::deposit_event(Event::<T>::ApplyClientRelease { release });
			reads += 1;
		}
		T::DbWeight::get().reads_writes(reads, reads * 2)
	}
}
