//! # Reward Distribution Module
//! A module for distributing rewards to Spacewalk vaults and their nominators.

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

mod default_weights;

pub use default_weights::{SubstrateWeight, WeightInfo};

use crate::types::{AccountIdOf, BalanceOf};
use frame_support::{
	dispatch::DispatchResult,
	traits::{Currency, Get},
	transactional, BoundedVec,
};
use sp_arithmetic::Perquintill;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

mod types;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use crate::*;

	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + security::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>>
			+ Into<<Self as frame_system::Config>::RuntimeEvent>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// The currency trait.
		type Currency: Currency<AccountIdOf<Self>>;

		/// Defines the interval (in number of blocks) at which the reward per block decays.
		#[pallet::constant]
		type DecayInterval: Get<Self::BlockNumber>;

		/// Defines the rate at which the reward per block decays.
		#[pallet::constant]
		type DecayRate: Get<Perquintill>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new RewardPerBlock value has been set.
		RewardsPerBlockAdapted(BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {}

	/// Reward per block.
	#[pallet::storage]
	#[pallet::getter(fn reward_per_block)]
	pub type RewardPerBlock<T: Config> = StorageValue<_, BalanceOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn rewards_adapted_at)]
	pub(super) type RewardsAdaptedAt<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the reward per block.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_rewards_per_block())]
		#[transactional]
		pub fn set_rewards_per_block(
			origin: OriginFor<T>,
			new_rewards_per_block: BalanceOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;

			RewardPerBlock::<T>::put(new_rewards_per_block);
			RewardsAdaptedAt::<T>::put(frame_system::Pallet::<T>::block_number());

			Self::deposit_event(Event::<T>::RewardsPerBlockAdapted(new_rewards_per_block));
			Ok(())
		}
	}
}
