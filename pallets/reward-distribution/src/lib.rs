//! # Reward Distribution Module
//! A module for distributing rewards to Spacewalk vaults and their nominators.

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

mod default_weights;

#[cfg(test)]
extern crate mocktopus;

use crate::types::{BalanceOf, DefaultVaultId};
use codec::{FullCodec, MaxEncodedLen};
use core::fmt::Debug;
use currency::{Amount, CurrencyId};
pub use default_weights::{SubstrateWeight, WeightInfo};
use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::DispatchError,
	traits::{Currency, Get},
	transactional, PalletId,
};
use oracle::OracleApi;
use orml_traits::GetByKey;
use sp_arithmetic::{traits::AtLeast32BitUnsigned, FixedPointOperand, Perquintill};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, CheckedSub, One, Zero},
	Saturating,
};
use sp_std::vec::Vec;
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
	pub trait Config:
		frame_system::Config + security::Config + currency::Config<Balance = BalanceOf<Self>>
	{
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>>
			+ Into<<Self as frame_system::Config>::RuntimeEvent>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// Balance Type
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

		/// Rewards API interface to connect with pooled-rewards
		type VaultRewards: pooled_rewards::RewardsApi<
			CurrencyId<Self>,
			DefaultVaultId<Self>,
			BalanceOf<Self>,
			CurrencyId = CurrencyId<Self>,
		>;

		/// Rewards API interface to connect with staking pallet
		type VaultStaking: staking::Staking<
			DefaultVaultId<Self>,
			Self::AccountId,
			Self::Index,
			BalanceOf<Self>,
			CurrencyId<Self>,
		>;
		/// Maximum allowed currencies
		type MaxCurrencies: Get<u32>;

		/// Fee pallet id from which fee account is derived
		type FeePalletId: Get<PalletId>;

		/// Oracle API adaptor
		type OracleApi: oracle::OracleApi<BalanceOf<Self>, CurrencyId<Self>>;

		/// Currency trait adaptor
		type Balances: Currency<Self::AccountId, Balance = BalanceOf<Self>>;

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
		/// Overflow
		Overflow,
		/// Underflow
		Underflow,
		/// Origin attempt to withdraw with 0 rewards
		NoRewardsForAccount,
		/// Amount to be minted is more than total rewarded
		NotEnoughRewardsRegistered,
		/// If distribution logic reaches an inconsistency with the amount of currencies in the
		/// system
		InconsistentRewardCurrencies,
		/// If the amount to collect is less than existential deposit
		CollectAmountTooSmall,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
			Self::execute_on_init(n);
			<T as Config>::WeightInfo::on_initialize()
		}
	}

	/// Reward per block.
	#[pallet::storage]
	#[pallet::getter(fn reward_per_block)]
	pub type RewardPerBlock<T: Config> = StorageValue<_, BalanceOf<T>, OptionQuery>;

	/// Last Block were rewards per block were modified
	#[pallet::storage]
	#[pallet::getter(fn rewards_adapted_at)]
	pub(super) type RewardsAdaptedAt<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

	/// Storage to keep track of the to-be-minted native rewards
	#[pallet::storage]
	#[pallet::getter(fn native_liability)]
	pub(super) type NativeLiability<T: Config> = StorageValue<_, BalanceOf<T>, OptionQuery>;

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
			RewardsAdaptedAt::<T>::put(ext::security::get_active_block::<T>());
			Self::deposit_event(Event::<T>::RewardPerBlockAdapted(new_reward_per_block));
			Ok(())
		}

		/// Allow users who have staked to collect rewards for a given vault and rewraded currency
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::collect_reward())]
		#[transactional]
		pub fn collect_reward(
			origin: OriginFor<T>,
			vault_id: DefaultVaultId<T>,
			reward_currency_id: CurrencyId<T>,
			index: Option<T::Index>,
		) -> DispatchResult {
			//distribute reward from reward pool to staking pallet store
			let caller = ensure_signed(origin)?;
			let reward = ext::pooled_rewards::withdraw_reward::<T>(
				&vault_id.collateral_currency(),
				&vault_id,
				reward_currency_id,
			)?;
			ext::staking::distribute_reward::<T>(&vault_id, reward, reward_currency_id)?;

			// We check if the amount to transfer is greater than of the existential deposit to
			// avoid potential losses of reward currency, in case currency is not native
			let minimum_transfer_amount =
				<T as orml_tokens::Config>::ExistentialDeposits::get(&reward_currency_id);
			let expected_rewards =
				ext::staking::compute_reward::<T>(&vault_id, &caller, reward_currency_id.clone())?;

			if expected_rewards == (BalanceOf::<T>::zero()) {
				return Err(Error::<T>::NoRewardsForAccount.into())
			}

			if expected_rewards < minimum_transfer_amount {
				return Err(Error::<T>::CollectAmountTooSmall.into())
			}

			//withdraw the reward for specific nominator
			let caller_rewards =
				ext::staking::withdraw_reward::<T>(&vault_id, &caller, index, reward_currency_id)?;

			if caller_rewards == (BalanceOf::<T>::zero()) {
				return Err(Error::<T>::NoRewardsForAccount.into())
			}

			//transfer rewards
			Self::transfer_reward(reward_currency_id, caller_rewards, caller)
		}
	}
}

impl<T: Config> Pallet<T> {
	fn transfer_reward(
		reward_currency_id: CurrencyId<T>,
		reward: BalanceOf<T>,
		beneficiary: T::AccountId,
	) -> DispatchResult {
		let native_currency_id = T::GetNativeCurrencyId::get();
		if reward_currency_id == native_currency_id {
			let available_native_funds = T::Balances::free_balance(&Self::fee_pool_account_id());

			match reward.checked_sub(&available_native_funds) {
				None => {
					// Pay the whole reward from the fee pool
					let amount: currency::Amount<T> = Amount::new(reward, reward_currency_id);
					return amount.transfer(&Self::fee_pool_account_id(), &beneficiary)
				},
				Some(remaining) => {
					//check if the to-be-minted amount is consistent
					//with reward tracking
					let liability = NativeLiability::<T>::get()
						.ok_or(Error::<T>::NotEnoughRewardsRegistered)?;

					if liability < remaining {
						return Err(Error::<T>::NotEnoughRewardsRegistered.into())
					}

					// Use the available funds from the fee pool
					let available_amount: currency::Amount<T> =
						Amount::new(available_native_funds, reward_currency_id);
					available_amount.transfer(&Self::fee_pool_account_id(), &beneficiary)?;
					// Mint the rest
					T::Balances::deposit_creating(&beneficiary, remaining);

					NativeLiability::<T>::set(Some(
						liability.checked_sub(&remaining).ok_or(Error::<T>::Underflow)?,
					));
				},
			}
			Ok(())
		} else {
			//we need no checking of available funds, since the fee pool will ALWAYS have enough
			// collected fees of the wrapped currencies
			let amount: currency::Amount<T> = Amount::new(reward, reward_currency_id);
			amount.transfer(&Self::fee_pool_account_id(), &beneficiary)
		}
	}

	pub fn execute_on_init(_height: T::BlockNumber) {
		if let Err(_) = ext::security::ensure_parachain_status_running::<T>() {
			return
		}

		//get reward per block
		let reward_per_block = match RewardPerBlock::<T>::get() {
			Some(value) => value,
			None => {
				log::warn!("Reward per block is None");
				return
			},
		};

		let rewards_adapted_at = match RewardsAdaptedAt::<T>::get() {
			Some(value) => value,
			None => {
				log::warn!("RewardsAdaptedAt is None");
				return
			},
		};

		let mut reward_this_block = reward_per_block;

		if Ok(true) ==
			ext::security::parachain_block_expired::<T>(
				rewards_adapted_at,
				T::DecayInterval::get().saturating_sub(T::BlockNumber::one()),
			) {
			let decay_rate = T::DecayRate::get();
			reward_this_block =
				(Perquintill::one().saturating_sub(decay_rate)).mul_floor(reward_per_block);
			RewardPerBlock::<T>::set(Some(reward_this_block));
			let active_block = ext::security::get_active_block::<T>();
			RewardsAdaptedAt::<T>::set(Some(active_block));
		} else {
			log::warn!("Failed to check if the parachain block expired");
		}
		//keep track of total native currencies to be minted
		NativeLiability::<T>::mutate(|current_liability| match current_liability {
			Some(current_liability) => *current_liability += reward_this_block,
			None => *current_liability = Some(reward_this_block),
		});

		if let Err(_) = Self::distribute_rewards(reward_this_block, T::GetNativeCurrencyId::get()) {
			log::warn!("Rewards distribution failed");
		}
	}

	//fetch total stake (of each pool), and calculate total USD stake in percentage across pools
	//distribute the reward accordingly
	fn distribute_rewards(
		reward_amount: BalanceOf<T>,
		reward_currency: CurrencyId<T>,
	) -> Result<BalanceOf<T>, DispatchError> {
		//calculate the total stake across all collateral pools in USD
		let total_stakes = ext::pooled_rewards::get_total_stake_all_pools::<T>()?;

		let mut total_stake_in_usd = BalanceOf::<T>::default();
		let mut stakes_in_usd = Vec::<BalanceOf<T>>::new();
		for (currency_id, stake) in total_stakes.clone().into_iter() {
			let stake_in_usd = T::OracleApi::currency_to_usd(&stake, &currency_id)?;
			stakes_in_usd.push(stake_in_usd);
			total_stake_in_usd = total_stake_in_usd.checked_add(&stake_in_usd).unwrap();
		}
		//distribute the rewards to each collateral pool
		let mut error_reward_accum = BalanceOf::<T>::zero();
		let mut stakes_in_usd_iter = stakes_in_usd.into_iter();
		for (currency_id, _stake) in total_stakes.into_iter() {
			let stake_in_usd =
				stakes_in_usd_iter.next().ok_or(Error::<T>::InconsistentRewardCurrencies)?;
			let percentage = Perquintill::from_rational(stake_in_usd, total_stake_in_usd);
			let reward_for_pool = percentage.mul_floor(reward_amount);
			if ext::pooled_rewards::distribute_reward::<T>(
				&currency_id,
				&reward_currency,
				reward_for_pool,
			)
			.is_err()
			{
				error_reward_accum =
					error_reward_accum.checked_add(&reward_for_pool).ok_or(Error::<T>::Overflow)?;
			}
		}

		Ok(error_reward_accum)
	}

	fn withdraw_all_rewards_from_vault(vault_id: DefaultVaultId<T>) -> DispatchResult {
		let all_reward_currencies = ext::staking::get_all_reward_currencies::<T>()?;
		for currency_id in all_reward_currencies {
			let reward = ext::pooled_rewards::withdraw_reward::<T>(
				&vault_id.collateral_currency(),
				&vault_id,
				currency_id,
			)?;
			ext::staking::distribute_reward::<T>(&vault_id, reward, currency_id)?;
		}
		Ok(())
	}

	pub fn fee_pool_account_id() -> T::AccountId {
		<T as Config>::FeePalletId::get().into_account_truncating()
	}
}
//Distribute Rewards interface
pub trait DistributeRewards<Balance, CurrencyId, VaultId> {
	fn distribute_rewards(
		amount: Balance,
		currency_id: CurrencyId,
	) -> Result<Balance, DispatchError>;

	fn withdraw_all_rewards_from_vault(vault_id: VaultId) -> DispatchResult;
}

impl<T: Config> DistributeRewards<BalanceOf<T>, CurrencyId<T>, DefaultVaultId<T>> for Pallet<T> {
	fn distribute_rewards(
		amount: BalanceOf<T>,
		currency_id: CurrencyId<T>,
	) -> Result<BalanceOf<T>, DispatchError> {
		Pallet::<T>::distribute_rewards(amount, currency_id)
	}

	fn withdraw_all_rewards_from_vault(vault_id: DefaultVaultId<T>) -> DispatchResult {
		Pallet::<T>::withdraw_all_rewards_from_vault(vault_id)
	}
}
