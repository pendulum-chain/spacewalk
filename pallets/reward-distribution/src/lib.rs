//! # Reward Distribution Module
//! A module for distributing rewards to Spacewalk vaults and their nominators.

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

mod default_weights;

use crate::types::{BalanceOf, DefaultVaultId};
use codec::{FullCodec, FullEncode, MaxEncodedLen};
pub use default_weights::{SubstrateWeight, WeightInfo};
use frame_support::{
	dispatch::DispatchResult, pallet_prelude::DispatchError, traits::Get, transactional, BoundedVec,
};
use oracle::OracleApi;
use pooled_rewards::RewardsApi;
use sp_arithmetic::Perquintill;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, One, Zero},
	FixedPointOperand,
};
use sp_std::{fmt::Debug, vec::Vec};
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
		type Currency: Parameter
			+ Member
			+ Copy
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

		//type OracleApi: ToUsdApi<Self::Balance, Self::Currency>;
		type OracleApi: oracle::OracleApi<Self::Balance, Self::Currency>;

		type GetNativeCurrencyId: Get<Self::Currency>;

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

		// #[pallet::call_index(1)]
		// #[pallet::weight(<T as Config>::WeightInfo::set_reward_per_block())]
		// #[transactional]
		// pub fn collect_rewards(
		// 	origin: OriginFor<T>,
		// 	collateral_asset: T::Currency,
		// ) -> DispatchResult {
		// 	let nominator_id = ensure_signed(origin)?;
		// }
	}
}

impl<T: Config> Pallet<T> {
	pub fn execute_on_init(height: T::BlockNumber) {
		//get reward per block
		let reward_per_block = match RewardPerBlock::<T>::get() {
			Some(value) => value,
			None => {
				log::warn!("Reward per block is None");
				return
			},
		};

		if let Err(_) = ext::security::ensure_parachain_status_running::<T>() {
			return
		}

		let rewards_adapted_at = match RewardsAdaptedAt::<T>::get() {
			Some(value) => value,
			None => {
				log::warn!("RewardsAdaptedAt is None");
				return
			},
		};

		let mut reward_this_block = reward_per_block;

		//update the reward per block if decay interval passed
		if let Ok(expired) = ext::security::parachain_block_expired::<T>(
			rewards_adapted_at,
			T::DecayInterval::get() - T::BlockNumber::one(),
		) {
			if expired {
				let decay_rate = T::DecayRate::get();
				reward_this_block = decay_rate.mul_floor(reward_per_block);
				RewardPerBlock::<T>::set(Some(reward_this_block));
				RewardsAdaptedAt::<T>::set(Some(height));
			}
		} else {
			log::warn!("Failed to check if the parachain block expired");
		}

		//TODO how to handle error if on init cannot fail?
		let _ = Self::distribute_rewards(reward_this_block, T::GetNativeCurrencyId::get());
	}

	//fetch total stake (all), and calulate total usd stake in percentage across pools
	//distribute the reward accoridigly
	fn distribute_rewards(
		reward_amount: BalanceOf<T>,
		reward_currency: T::Currency,
	) -> Result<BalanceOf<T>, DispatchError> {
		//calculate the total stake across all collateral pools in USD
		let total_stakes = T::VaultRewards::get_total_stake_all_pools()?;
		let total_stake_in_usd = BalanceOf::<T>::default();
		for (currency_id, stake) in total_stakes.clone().into_iter() {
			let stake_in_usd = T::OracleApi::currency_to_usd(&stake, &currency_id)?;
			total_stake_in_usd.checked_add(&stake_in_usd).unwrap();
		}

		//distribute the rewards to each collateral pool
		let mut percentages_vec = Vec::<(T::Currency, Perquintill)>::new();
		let mut error_reward_accum = BalanceOf::<T>::zero();
		for (currency_id, stake) in total_stakes.into_iter() {
			let stake_in_usd = T::OracleApi::currency_to_usd(&stake, &currency_id)?;
			let percentage = Perquintill::from_rational(stake_in_usd, total_stake_in_usd);

			let reward_for_pool = percentage.mul_floor(reward_amount);

			if T::VaultRewards::distribute_reward(
				&currency_id,
				reward_currency.clone(),
				reward_for_pool,
			)
			.is_err()
			{
				error_reward_accum =
					error_reward_accum.checked_add(&reward_for_pool).ok_or(Error::<T>::Overflow)?;
			}

			percentages_vec.push((currency_id, percentage))
		}

		//we store the calculated percentages which are good as long as
		//prices are unchanged
		let bounded_vec = BoundedVec::try_from(percentages_vec)
			.expect("should not be longer than max currencies");
		RewardsPercentage::<T>::set(Some(bounded_vec));

		Ok(error_reward_accum)
	}
}
//Distribute Rewards interface
pub trait DistributeRewardsToPool<Balance, CurrencyId> {
	fn distribute_rewards(
		amount: Balance,
		currency_id: CurrencyId,
	) -> Result<Balance, DispatchError>;
}

impl<T: Config> DistributeRewardsToPool<BalanceOf<T>, T::Currency> for Pallet<T> {
	fn distribute_rewards(
		amount: BalanceOf<T>,
		currency_id: T::Currency,
	) -> Result<BalanceOf<T>, DispatchError> {
		Pallet::<T>::distribute_rewards(amount, currency_id)
	}
}
