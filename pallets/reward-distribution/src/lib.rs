//! # Reward Distribution Module
//! A module for distributing rewards to Spacewalk vaults and their nominators.

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

mod default_weights;

pub use default_weights::{SubstrateWeight, WeightInfo};

use crate::types::{AccountIdOf, BalanceOf, DefaultVaultId};
use codec::{FullCodec, MaxEncodedLen};
use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::DispatchError,
	traits::{Currency, Get},
	transactional,
};
use pooled_rewards::RewardsApi;
use primitives::CurrencyId;
use sp_arithmetic::Perquintill;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, One},
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
		type Currency: Currency<AccountIdOf<Self>> + Clone;

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
	pub enum Error<T> {}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
			Self::distribute_rewards(n);
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
		BoundedVec<(CurrencyId, Perquintill), <T as pallet::Config>::MaxCurrencies>,
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
	pub fn distribute_rewards(height: T::BlockNumber) {
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
			rewards_adapted_at.expect("checked for none"),
			T::DecayInterval::get() - T::BlockNumber::one(),
		)
		.unwrap()
		{
			let decay_rate = T::DecayRate::get();
			let reward_this_block =
				decay_rate.mul_floor(reward_per_block.expect("checked for none"));
			RewardPerBlock::<T>::set(Some(new_reward_per_block));

			RewardsAdaptedAt::<T>::set(Some(height));
		}

		let total_reward_stake = Self::caculate_reward_stake_percentages(reward_this_block).unwrap();

	
	}

	//fetch total stake (all), and calulate total usd stake in percentage across pools
	fn caculate_reward_stake_percentages() -> Result<(), ()> {
		let total_stakes = T::VaultRewards::get_total_stake_all_pools().unwrap();
		let total_stake_in_usd = BalanceOf::<T>::default();
		for (currency_id, stake) in total_stakes.clone().into_iter() {
			let stake_in_usd = T::OracleApi::currency_to_usd(stake, currency_id).unwrap();
			total_stake_in_usd.checked_add(&stake_in_usd).unwrap();
		}
		Ok(total_stake_in_usd)

		let percentages_vec: Vec::new();
		for (currency_id, stake) in total_stakes.into_iter() {
			let stake_in_amount = Amount::<T>::new(stake, currency_id);
			let stake_in_usd = stake_in_amount.convert_to(<T as Config>::BaseCurrency::get())?;
			let percentage = Perquintill::from_rational(stake_in_usd.amount(), total_stake_in_usd);

			percentages_vec.push((currency_id, percentage))
		}

		let bounded_vec = BoundedVec::try_from(percentages_vec).expect("should not be longer than max currencies");

		RewardsPercentage::<T>::set(bounded_vec);
	}

	pub fn distribute_rewards(reward)-> Result<(),(){
		// multiply with floor or ceil?

		let stake_percentages = RewardsPercentage::<T>::get();
		
		for (currency_id, stake) in stake_percentages.into_iter() {
			let reward_for_pool = percentage.mul_floor(reward.amount());
			let error_reward_accum = Amount::<T>::zero(reward.currency());
			if T::VaultRewards::distribute_reward(&currency_id, reward.currency(), reward_for_pool)
				.is_err()
			{
				error_reward_accum.checked_add(&reward)?;
			}
		}
		
	}

}



pub trait ToUsdApi<Balance, CurrencyId> {
	fn currency_to_usd(amount: Balance, currency_id: CurrencyId) -> Result<Balance, DispatchError>;
}
