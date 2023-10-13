//! # Reward Distribution Module
//! A module for distributing rewards to Spacewalk vaults and their nominators.

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

mod default_weights;

use crate::types::{AccountIdOf, BalanceOf, DefaultVaultId};
use codec::{FullCodec, FullEncode, MaxEncodedLen};
pub use default_weights::{SubstrateWeight, WeightInfo};
use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::DispatchError,
	traits::{Currency, Get},
	transactional,
};
use pooled_rewards::RewardsApi;
use sp_arithmetic::Perquintill;
use sp_core::bounded_vec::BoundedVec;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, One, Zero},
	FixedPointOperand,
};
use sp_std::fmt::Debug;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod ext;
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
		type Currency: Currency<AccountIdOf<Self>>
			+ Clone
			+ Decode
			+ FullEncode
			+ TypeInfo
			+ MaxEncodedLen
			+ Debug;

		//balance
		type Balance: AtLeast32BitUnsigned
			+ TypeInfo
			+ FixedPointOperand
			+ MaybeSerializeDeserialize
			+ FullCodec
			+ MaxEncodedLen
			+ Copy
			+ Default
			+ Debug
			+ Clone
			+ From<u64>;

		type VaultRewards: pooled_rewards::RewardsApi<
			Self::Currency,
			DefaultVaultId<Self>,
			BalanceOf<Self>,
			Self::Currency,
		>;

		type MaxCurrencies: Get<u32>;

		type OracleApi: ToUsdApi<Self::Balance, Self::Currency>;

		type NativeToken: Get<Self::Currency>;

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
		RewardPerBlockAdapted(BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		//Overflow
		Overflow,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
			Self::execute_on_init(n);
			//TODO benchmark this weight properly
			<T as Config>::WeightInfo::on_initialize()
		}
	}

	/// Reward per block.
	#[pallet::storage]
	#[pallet::getter(fn reward_per_block)]
	pub type RewardPerBlock<T: Config> = StorageValue<_, BalanceOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn rewards_adapted_at)]
	pub(super) type RewardsAdaptedAt<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn rewards_percentage)]
	pub(super) type RewardsPercentage<T: Config> = StorageValue<
		_,
		BoundedVec<
			(<T as pallet::Config>::Currency, Perquintill),
			<T as pallet::Config>::MaxCurrencies,
		>,
		OptionQuery,
	>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the reward per block.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_reward_per_block())]
		#[transactional]
		pub fn set_reward_per_block(
			origin: OriginFor<T>,
			new_reward_per_block: BalanceOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;

			RewardPerBlock::<T>::put(new_reward_per_block);
			RewardsAdaptedAt::<T>::put(frame_system::Pallet::<T>::block_number());

			Self::deposit_event(Event::<T>::RewardPerBlockAdapted(new_reward_per_block));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn execute_on_init(height: T::BlockNumber) {
		//get reward per block
		let reward_per_block = RewardPerBlock::<T>::get();
		if reward_per_block == None {
			return
		}

		if let Err(_) = ext::security::ensure_parachain_status_running::<T>() {
			return
		}

		let mut reward_this_block = reward_per_block.expect("checked for none");
		//update the reward per block if decay interval passed
		let rewards_adapted_at = RewardsAdaptedAt::<T>::get();
		if ext::security::parachain_block_expired::<T>(
			rewards_adapted_at.expect("HANDLE"),
			T::DecayInterval::get() - T::BlockNumber::one(),
		)
		.unwrap()
		{
			let decay_rate = T::DecayRate::get();
			reward_this_block = decay_rate.mul_floor(reward_per_block.expect("checked for none"));
			RewardPerBlock::<T>::set(Some(reward_this_block));

			RewardsAdaptedAt::<T>::set(Some(height));
		}

		//TODO how to handle error if on init cannot fail?
		let _ = Self::distribute_rewards(T::NativeToken::get(), reward_this_block);
	}

	//fetch total stake (all), and calulate total usd stake in percentage across pools
	fn distribute_rewards(
		reward_currency: T::Currency,
		reward_amount: BalanceOf<T>,
	) -> Result<(), DispatchError> {
		let total_stakes = T::VaultRewards::get_total_stake_all_pools().unwrap();
		let total_stake_in_usd = BalanceOf::<T>::default();
		for (currency_id, stake) in total_stakes.clone().into_iter() {
			let stake_in_usd = T::OracleApi::currency_to_usd(&stake, &currency_id).unwrap();
			total_stake_in_usd.checked_add(&stake_in_usd).unwrap();
		}

		let mut percentages_vec = Vec::<(T::Currency, Perquintill)>::new();
		for (currency_id, stake) in total_stakes.into_iter() {
			let stake_in_usd = T::OracleApi::currency_to_usd(&stake, &currency_id)?;
			let percentage = Perquintill::from_rational(stake_in_usd, total_stake_in_usd);

			let reward_for_pool = percentage.mul_floor(reward_amount);
			let error_reward_accum = BalanceOf::<T>::zero();
			if T::VaultRewards::distribute_reward(
				&currency_id,
				reward_currency.clone(),
				reward_for_pool,
			)
			.is_err()
			{
				error_reward_accum.checked_add(&reward_for_pool).ok_or(Error::<T>::Overflow)?;
			}

			percentages_vec.push((currency_id, percentage))
		}

		//we store the calculated percentages which are good as long as
		//prices are unchanged
		let bounded_vec = BoundedVec::try_from(percentages_vec)
			.expect("should not be longer than max currencies");
		RewardsPercentage::<T>::set(Some(bounded_vec));

		Ok(())
	}
}

//TODO I actually want this api coming form oracle (defined and implemented)
//but importing oracle gives an unexpected and unrelated error
pub trait ToUsdApi<Balance, CurrencyId> {
	fn currency_to_usd(
		amount: &Balance,
		currency_id: &CurrencyId,
	) -> Result<Balance, DispatchError>;
}
