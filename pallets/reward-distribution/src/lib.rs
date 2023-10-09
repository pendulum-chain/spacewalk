//! # Reward Distribution Module
//! A module for distributing rewards to Spacewalk vaults and their nominators.

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
			ensure_root(origin)?;
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
			ensure_root(origin)?;
			PendingClientReleases::<T>::insert(client_name, release.clone());
			Self::deposit_event(Event::<T>::NotifyClientRelease { release });
			Ok(())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NotifyClientRelease { release: ClientRelease<UriOf<T>, T::Hash> },
		ApplyClientRelease { release: ClientRelease<UriOf<T>, T::Hash> },
	}

	#[pallet::error]
	pub enum Error<T> {}

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
