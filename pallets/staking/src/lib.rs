//! # Staking Module
//! Based on the [Scalable Reward Distribution](https://solmaz.io/2019/02/24/scalable-reward-changing/) algorithm.

// Below is a simplified model of this code. It is accurate, but for simplicity it uses
// floats, and only has a single account (without nomination) and currency. It's useful
// as a mental model for the more complicated implementation.
//
// struct State {
//     reward_per_token: f64,
//     reward_tally: f64,
//     slash_per_token: f64,
//     slash_tally: f64,
//     stake: f64,
//     total_current_stake: f64,
//     total_stake: f64,
// }
// impl State {
//     fn apply_slash(&mut self) {
//         self.total_stake -= self.stake * self.slash_per_token - self.slash_tally;
//         self.stake -= self.stake * self.slash_per_token - self.slash_tally;
//         self.slash_tally = self.stake * self.slash_per_token;
//     }
//     fn distribute_reward(&mut self, x: f64) {
//         self.reward_per_token += x / self.total_current_stake;
//     }
//     fn withdraw_reward(&mut self) -> f64 {
//         self.apply_slash();
//         let withdrawal_reward = self.stake * self.reward_per_token - self.reward_tally;
//         self.reward_tally = self.stake * self.reward_per_token;
//         withdrawal_reward
//     }
//     fn deposit_stake(&mut self, x: f64) {
//         self.apply_slash();
//         self.stake += x;
//         self.total_stake += x;
//         self.total_current_stake += x;
//         self.slash_tally += self.slash_per_token * x;
//         self.reward_tally += self.reward_per_token * x;
//
//         self.reward_per_token += x / self.total_current_stake;
//     }
//     fn withdraw_stake(&mut self, x: f64) {
//         self.deposit_stake(-x)
//     }
//     fn slash_stake(&mut self, x: f64) {
//         self.slash_per_token += x / self.total_stake;
//         self.total_current_stake -= x;
//         self.reward_per_token += (self.reward_per_token * x) / self.total_current_stake;
//     }
// }

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::EncodeLike;
use frame_support::{dispatch::DispatchResult, sp_runtime, traits::Get};

use sp_arithmetic::{FixedPointNumber, FixedPointOperand};
use sp_runtime::{
	traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One, Zero},
	ArithmeticError, DispatchError,
};
use sp_std::{cmp, convert::TryInto};

pub use pallet::*;
use primitives::{BalanceToFixedPoint, TruncateFixedPointToInt, VaultCurrencyPair, VaultId};
use sp_std::vec::Vec;
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub(crate) type SignedFixedPoint<T> = <T as Config>::SignedFixedPoint;

pub type DefaultVaultId<T> =
	VaultId<<T as frame_system::Config>::AccountId, <T as Config>::CurrencyId>;
pub type DefaultVaultCurrencyPair<T> = VaultCurrencyPair<<T as Config>::CurrencyId>;
pub type NominatorId<T> = <T as frame_system::Config>::AccountId;
#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use frame_support::{pallet_prelude::*, BoundedBTreeSet};
	use frame_system::pallet_prelude::BlockNumberFor;
	/// ## Configuration
	/// The pallet's configuration trait.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The `Inner` type of the `SignedFixedPoint`.
		type SignedInner: CheckedDiv + Ord + FixedPointOperand;

		/// Signed fixed point type.
		type SignedFixedPoint: FixedPointNumber<Inner = Self::SignedInner>
			+ TruncateFixedPointToInt
			+ Encode
			+ EncodeLike
			+ Decode
			+ TypeInfo;

		#[pallet::constant]
		type GetNativeCurrencyId: Get<Self::CurrencyId>;

		/// The currency ID type.
		type CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord;

		/// Maximum reward currencies allowed
		type MaxRewardCurrencies: Get<u32>;
	}

	// The pallet's events
	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		DepositStake {
			vault_id: DefaultVaultId<T>,
			nominator_id: T::AccountId,
			amount: T::SignedFixedPoint,
		},
		DistributeReward {
			currency_id: T::CurrencyId,
			vault_id: DefaultVaultId<T>,
			amount: T::SignedFixedPoint,
		},
		WithdrawStake {
			vault_id: DefaultVaultId<T>,
			nominator_id: T::AccountId,
			amount: T::SignedFixedPoint,
		},
		WithdrawReward {
			nonce: T::Nonce,
			currency_id: T::CurrencyId,
			vault_id: DefaultVaultId<T>,
			nominator_id: T::AccountId,
			amount: T::SignedFixedPoint,
		},
		ForceRefund {
			vault_id: DefaultVaultId<T>,
		},
		IncreaseNonce {
			vault_id: DefaultVaultId<T>,
			new_nonce: T::Nonce,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Unable to convert value.
		TryIntoIntError,
		/// Balance not sufficient to withdraw stake.
		InsufficientFunds,
		/// Cannot slash zero total stake.
		SlashZeroTotalStake,
		/// Max rewards currencies threshold
		MaxRewardCurrencies,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// The total stake - this will increase on deposit and decrease on withdrawal.
	#[pallet::storage]
	#[pallet::getter(fn total_stake_at_index)]
	pub type TotalStake<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Nonce,
		Blake2_128Concat,
		DefaultVaultId<T>,
		SignedFixedPoint<T>,
		ValueQuery,
	>;

	/// The total stake - this will increase on deposit and decrease on withdrawal or slashing.
	#[pallet::storage]
	#[pallet::getter(fn total_current_stake_at_index)]
	pub type TotalCurrentStake<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Nonce,
		Blake2_128Concat,
		DefaultVaultId<T>,
		SignedFixedPoint<T>,
		ValueQuery,
	>;

	/// The total unclaimed rewards distributed to this reward pool.
	/// NOTE: this is currently only used for integration tests.
	// TODO: conditionally compile this
	#[pallet::storage]
	#[pallet::getter(fn total_rewards)]
	pub type TotalRewards<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::CurrencyId,
		Blake2_128Concat,
		(T::Nonce, DefaultVaultId<T>),
		SignedFixedPoint<T>,
		ValueQuery,
	>;

	/// Used to compute the rewards for a participant's stake.
	#[pallet::storage]
	#[pallet::getter(fn reward_per_token)]
	pub type RewardPerToken<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::CurrencyId,
		Blake2_128Concat,
		(T::Nonce, DefaultVaultId<T>),
		SignedFixedPoint<T>,
		ValueQuery,
	>;

	/// Used to compute the amount to slash from a participant's stake.
	#[pallet::storage]
	pub type SlashPerToken<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Nonce,
		Blake2_128Concat,
		DefaultVaultId<T>,
		SignedFixedPoint<T>,
		ValueQuery,
	>;

	/// The stake of a participant in this reward pool.
	#[pallet::storage]
	pub type Stake<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Nonce,
		Blake2_128Concat,
		(DefaultVaultId<T>, T::AccountId),
		SignedFixedPoint<T>,
		ValueQuery,
	>;

	/// Accounts for previous changes in stake size.
	#[pallet::storage]
	pub type RewardTally<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::CurrencyId,
		Blake2_128Concat,
		(T::Nonce, DefaultVaultId<T>, T::AccountId),
		SignedFixedPoint<T>,
		ValueQuery,
	>;

	/// Accounts for previous changes in stake size.
	#[pallet::storage]
	pub type SlashTally<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Nonce,
		Blake2_128Concat,
		(DefaultVaultId<T>, T::AccountId),
		SignedFixedPoint<T>,
		ValueQuery,
	>;

	/// The nonce of the current staking pool, used in force refunds.
	/// This is a strictly increasing value.
	#[pallet::storage]
	pub type Nonce<T: Config> =
		StorageMap<_, Blake2_128Concat, DefaultVaultId<T>, T::Nonce, ValueQuery>;

	/// store with all the reward currencies in use
	#[pallet::storage]
	pub type RewardCurrencies<T: Config> =
		StorageValue<_, BoundedBTreeSet<T::CurrencyId, T::MaxRewardCurrencies>, ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info] // no MaxEncodedLen for <T as frame_system::Config>::Index
	pub struct Pallet<T>(_);

	// The pallet's dispatchable functions.
	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

macro_rules! checked_add_mut {
	($storage:ty, $currency:expr, $amount:expr) => {
		<$storage>::mutate($currency, |value| {
			*value = value.checked_add($amount).ok_or(ArithmeticError::Overflow)?;
			Ok::<_, DispatchError>(*value)
		})?
	};
	($storage:ty, $currency:expr, $account:expr, $amount:expr) => {
		<$storage>::mutate($currency, $account, |value| {
			*value = value.checked_add($amount).ok_or(ArithmeticError::Overflow)?;
			Ok::<_, DispatchError>(*value)
		})?
	};
}

macro_rules! checked_sub_mut {
	($storage:ty, $currency:expr, $amount:expr) => {
		<$storage>::mutate($currency, |value| {
			*value = value.checked_sub($amount).ok_or(ArithmeticError::Underflow)?;
			Ok::<_, DispatchError>(*value)
		})?
	};
	($storage:ty, $currency:expr, $account:expr, $amount:expr) => {
		<$storage>::mutate($currency, $account, |value| {
			*value = value.checked_sub($amount).ok_or(ArithmeticError::Underflow)?;
			Ok::<_, DispatchError>(*value)
		})?
	};
}

// "Internal" functions, callable by code.
impl<T: Config> Pallet<T> {
	/// Get the stake associated with a vault / nominator.
	pub fn stake(vault_id: &DefaultVaultId<T>, nominator_id: &T::AccountId) -> SignedFixedPoint<T> {
		let nonce = Self::nonce(vault_id);
		Self::stake_at_index(nonce, vault_id, nominator_id)
	}

	fn stake_at_index(
		nonce: T::Nonce,
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> SignedFixedPoint<T> {
		<Stake<T>>::get(nonce, (vault_id, nominator_id))
	}

	/// Get the total stake *after* slashing.
	pub fn total_current_stake(
		vault_id: &DefaultVaultId<T>,
	) -> Result<<SignedFixedPoint<T> as FixedPointNumber>::Inner, DispatchError> {
		let nonce = Self::nonce(vault_id);
		let total = Self::total_current_stake_at_index(nonce, vault_id);
		total.truncate_to_inner().ok_or_else(|| Error::<T>::TryIntoIntError.into())
	}

	fn reward_tally(
		nonce: T::Nonce,
		currency_id: T::CurrencyId,
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> SignedFixedPoint<T> {
		<RewardTally<T>>::get(currency_id, (nonce, vault_id, nominator_id))
	}

	/// Get the nominator's `slash_tally` for the staking pool.
	pub fn slash_tally(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> SignedFixedPoint<T> {
		let nonce = Self::nonce(vault_id);
		Self::slash_tally_at_index(nonce, vault_id, nominator_id)
	}

	fn slash_tally_at_index(
		nonce: T::Nonce,
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> SignedFixedPoint<T> {
		<SlashTally<T>>::get(nonce, (vault_id, nominator_id))
	}

	/// Get the newest nonce for the staking pool.
	pub fn nonce(vault_id: &DefaultVaultId<T>) -> T::Nonce {
		<Nonce<T>>::get(vault_id)
	}

	/// Get the vault's `slash_per_token` for the staking pool.
	pub fn slash_per_token(vault_id: &DefaultVaultId<T>) -> SignedFixedPoint<T> {
		let nonce = Self::nonce(vault_id);
		Self::slash_per_token_at_index(nonce, vault_id)
	}

	fn slash_per_token_at_index(
		nonce: T::Nonce,
		vault_id: &DefaultVaultId<T>,
	) -> SignedFixedPoint<T> {
		<SlashPerToken<T>>::get(nonce, vault_id)
	}

	/// Deposit an `amount` of stake to the `vault_id` for the `nominator_id`.
	pub fn deposit_stake(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		amount: SignedFixedPoint<T>,
	) -> DispatchResult {
		let nonce = Self::nonce(vault_id);
		Self::apply_slash(vault_id, nominator_id)?;

		checked_add_mut!(Stake<T>, nonce, (vault_id, nominator_id), &amount);
		checked_add_mut!(TotalStake<T>, nonce, vault_id, &amount);
		checked_add_mut!(TotalCurrentStake<T>, nonce, vault_id, &amount);

		<SlashTally<T>>::mutate(nonce, (vault_id, nominator_id), |slash_tally| {
			let slash_per_token = Self::slash_per_token_at_index(nonce, vault_id);
			let slash_per_token_mul_amount =
				slash_per_token.checked_mul(&amount).ok_or(ArithmeticError::Overflow)?;
			*slash_tally = slash_tally
				.checked_add(&slash_per_token_mul_amount)
				.ok_or(ArithmeticError::Overflow)?;
			Ok::<_, DispatchError>(())
		})?;
		let all_reward_currencies = Self::get_all_reward_currencies()?;
		for currency_id in all_reward_currencies {
			<RewardTally<T>>::mutate(
				currency_id,
				(nonce, vault_id, nominator_id),
				|reward_tally| {
					let reward_per_token = Self::reward_per_token(currency_id, (nonce, vault_id));
					let reward_per_token_mul_amount =
						reward_per_token.checked_mul(&amount).ok_or(ArithmeticError::Overflow)?;
					*reward_tally = reward_tally
						.checked_add(&reward_per_token_mul_amount)
						.ok_or(ArithmeticError::Overflow)?;
					Ok::<_, DispatchError>(())
				},
			)?;
		}

		Self::deposit_event(Event::<T>::DepositStake {
			vault_id: vault_id.clone(),
			nominator_id: nominator_id.clone(),
			amount,
		});
		Ok(())
	}

	#[cfg(test)]
	// will be used to test migration
	fn broken_slash_stake_do_not_use(
		currency_id: T::CurrencyId,
		vault_id: &DefaultVaultId<T>,
		amount: SignedFixedPoint<T>,
	) -> DispatchResult {
		let nonce = Self::nonce(vault_id);
		let total_stake = Self::total_stake_at_index(nonce, vault_id);
		if amount.is_zero() {
			return Ok(());
		} else if total_stake.is_zero() {
			return Err(Error::<T>::SlashZeroTotalStake.into());
		}

		let amount_div_total_stake =
			amount.checked_div(&total_stake).ok_or(ArithmeticError::Underflow)?;
		checked_add_mut!(SlashPerToken<T>, nonce, vault_id, &amount_div_total_stake);

		checked_sub_mut!(TotalCurrentStake<T>, nonce, vault_id, &amount);

		Self::increase_rewards(
			nonce,
			currency_id,
			vault_id,
			Self::reward_per_token(currency_id, (nonce, vault_id))
				.checked_mul(&amount)
				.ok_or(ArithmeticError::Overflow)?,
		)?;
		Ok(())
	}

	/// Slash an `amount` of stake from the `vault_id`.
	pub fn slash_stake(
		vault_id: &DefaultVaultId<T>,
		amount: SignedFixedPoint<T>,
	) -> DispatchResult {
		let nonce = Self::nonce(vault_id);
		let total_stake = Self::total_stake_at_index(nonce, vault_id);
		if amount.is_zero() {
			return Ok(());
		} else if total_stake.is_zero() {
			return Err(Error::<T>::SlashZeroTotalStake.into());
		}

		let amount_div_total_stake =
			amount.checked_div(&total_stake).ok_or(ArithmeticError::Underflow)?;
		checked_add_mut!(SlashPerToken<T>, nonce, vault_id, &amount_div_total_stake);

		checked_sub_mut!(TotalCurrentStake<T>, nonce, vault_id, &amount);

		// A slash means reward per token is no longer representative of the rewards
		// since `amount * reward_per_token` will be lost from the system. As such,
		// replenish rewards by the amount of reward lost with this slash
		let all_reward_currencies = Self::get_all_reward_currencies()?;
		for currency_id in all_reward_currencies {
			Self::increase_rewards(
				nonce,
				currency_id,
				vault_id,
				Self::reward_per_token(currency_id, (nonce, vault_id))
					.checked_mul(&amount)
					.ok_or(ArithmeticError::Overflow)?,
			)?;
		}
		Ok(())
	}

	fn compute_amount_to_slash(
		stake: SignedFixedPoint<T>,
		slash_per_token: SignedFixedPoint<T>,
		slash_tally: SignedFixedPoint<T>,
	) -> Result<SignedFixedPoint<T>, DispatchError> {
		let stake_mul_slash_per_token =
			stake.checked_mul(&slash_per_token).ok_or(ArithmeticError::Overflow)?;

		let to_slash = stake_mul_slash_per_token
			.checked_sub(&slash_tally)
			.ok_or(ArithmeticError::Underflow)?;

		Ok(to_slash)
	}

	/// Delegates to `compute_stake_at_index` with the current nonce.
	pub fn compute_stake(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> Result<<SignedFixedPoint<T> as FixedPointNumber>::Inner, DispatchError> {
		let nonce = Self::nonce(vault_id);
		Self::compute_stake_at_index(nonce, vault_id, nominator_id)
	}

	/// Compute the stake in `vault_id` owned by `nominator_id`.
	pub fn compute_stake_at_index(
		nonce: T::Nonce,
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> Result<<SignedFixedPoint<T> as FixedPointNumber>::Inner, DispatchError> {
		let stake = Self::stake_at_index(nonce, vault_id, nominator_id);
		let slash_per_token = Self::slash_per_token_at_index(nonce, vault_id);
		let slash_tally = Self::slash_tally_at_index(nonce, vault_id, nominator_id);
		let to_slash = Self::compute_amount_to_slash(stake, slash_per_token, slash_tally)?;

		let stake_sub_to_slash = stake
			.checked_sub(&to_slash)
			.ok_or(ArithmeticError::Underflow)?
			.truncate_to_inner()
			.ok_or(Error::<T>::TryIntoIntError)?;
		Ok(cmp::max(Zero::zero(), stake_sub_to_slash))
	}

	fn increase_rewards(
		nonce: T::Nonce,
		currency_id: T::CurrencyId,
		vault_id: &DefaultVaultId<T>,
		reward: SignedFixedPoint<T>,
	) -> Result<SignedFixedPoint<T>, DispatchError> {
		let total_current_stake = Self::total_current_stake_at_index(nonce, vault_id);
		if total_current_stake.is_zero() {
			return Ok(Zero::zero());
		}

		let reward_div_total_current_stake =
			reward.checked_div(&total_current_stake).ok_or(ArithmeticError::Underflow)?;
		checked_add_mut!(
			RewardPerToken<T>,
			currency_id,
			(nonce, vault_id),
			&reward_div_total_current_stake
		);
		Ok(reward)
	}

	/// Distribute the `reward` to all participants.
	pub fn distribute_reward(
		currency_id: T::CurrencyId,
		vault_id: &DefaultVaultId<T>,
		reward: SignedFixedPoint<T>,
	) -> Result<SignedFixedPoint<T>, DispatchError> {
		let nonce = Self::nonce(vault_id);

		let reward = Self::increase_rewards(nonce, currency_id, vault_id, reward)?;
		if reward.is_zero() {
			return Ok(Zero::zero());
		}
		checked_add_mut!(TotalRewards<T>, currency_id, (nonce, vault_id), &reward);

		Self::deposit_event(Event::<T>::DistributeReward {
			currency_id,
			vault_id: vault_id.clone(),
			amount: reward,
		});
		Ok(reward)
	}

	/// Delegates to `compute_reward_at_index` with the current nonce.
	pub fn compute_reward(
		currency_id: T::CurrencyId,
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> Result<<SignedFixedPoint<T> as FixedPointNumber>::Inner, DispatchError> {
		let nonce = Self::nonce(vault_id);
		Self::compute_reward_at_index(nonce, currency_id, vault_id, nominator_id)
	}

	/// Compute the expected reward for `nominator_id` who is nominating `vault_id`.
	pub fn compute_reward_at_index(
		nonce: T::Nonce,
		currency_id: T::CurrencyId,
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> Result<<SignedFixedPoint<T> as FixedPointNumber>::Inner, DispatchError> {
		let stake = SignedFixedPoint::<T>::checked_from_integer(Self::compute_stake_at_index(
			nonce,
			vault_id,
			nominator_id,
		)?)
		.ok_or(Error::<T>::TryIntoIntError)?;
		let reward_per_token = Self::reward_per_token(currency_id, (nonce, vault_id));
		// FIXME: this can easily overflow with large numbers
		let stake_mul_reward_per_token =
			stake.checked_mul(&reward_per_token).ok_or(ArithmeticError::Overflow)?;
		let reward_tally = Self::reward_tally(nonce, currency_id, vault_id, nominator_id);
		// TODO: this can probably be saturated
		let reward = stake_mul_reward_per_token
			.checked_sub(&reward_tally)
			.ok_or(ArithmeticError::Underflow)?
			.truncate_to_inner()
			.ok_or(Error::<T>::TryIntoIntError)?;
		Ok(cmp::max(Zero::zero(), reward))
	}

	fn apply_slash(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> Result<SignedFixedPoint<T>, DispatchError> {
		let nonce = Self::nonce(vault_id);
		let stake = Self::stake_at_index(nonce, vault_id, nominator_id);
		let slash_per_token = Self::slash_per_token_at_index(nonce, vault_id);
		let slash_tally = Self::slash_tally_at_index(nonce, vault_id, nominator_id);
		let to_slash = Self::compute_amount_to_slash(stake, slash_per_token, slash_tally)?;

		checked_sub_mut!(TotalStake<T>, nonce, vault_id, &to_slash);

		let stake = checked_sub_mut!(Stake<T>, nonce, (vault_id, nominator_id), &to_slash);
		<SlashTally<T>>::insert(
			nonce,
			(vault_id, nominator_id),
			stake.checked_mul(&slash_per_token).ok_or(ArithmeticError::Overflow)?,
		);

		Ok(stake)
	}

	/// Withdraw an `amount` of stake from the `vault_id` for the `nominator_id`.
	pub fn withdraw_stake(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		amount: SignedFixedPoint<T>,
		index: Option<T::Nonce>,
	) -> DispatchResult {
		let nonce = index.unwrap_or_else(|| Self::nonce(vault_id));
		let stake = Self::apply_slash(vault_id, nominator_id)?;

		if amount.is_zero() {
			return Ok(());
		} else if amount > stake {
			return Err(Error::<T>::InsufficientFunds.into());
		}

		checked_sub_mut!(Stake<T>, nonce, (vault_id, nominator_id), &amount);
		checked_sub_mut!(TotalStake<T>, nonce, vault_id, &amount);
		checked_sub_mut!(TotalCurrentStake<T>, nonce, vault_id, &amount);

		<SlashTally<T>>::mutate(nonce, (vault_id, nominator_id), |slash_tally| {
			let slash_per_token = Self::slash_per_token_at_index(nonce, vault_id);
			let slash_per_token_mul_amount =
				slash_per_token.checked_mul(&amount).ok_or(ArithmeticError::Overflow)?;

			*slash_tally = slash_tally
				.checked_sub(&slash_per_token_mul_amount)
				.ok_or(ArithmeticError::Underflow)?;
			Ok::<_, DispatchError>(())
		})?;

		let all_reward_currencies = Self::get_all_reward_currencies()?;
		for currency_id in all_reward_currencies {
			<RewardTally<T>>::mutate(
				currency_id,
				(nonce, vault_id, nominator_id),
				|reward_tally| {
					let reward_per_token = Self::reward_per_token(currency_id, (nonce, vault_id));
					let reward_per_token_mul_amount =
						reward_per_token.checked_mul(&amount).ok_or(ArithmeticError::Overflow)?;

					*reward_tally = reward_tally
						.checked_sub(&reward_per_token_mul_amount)
						.ok_or(ArithmeticError::Underflow)?;
					Ok::<_, DispatchError>(())
				},
			)?;
		}

		Self::deposit_event(Event::<T>::WithdrawStake {
			vault_id: vault_id.clone(),
			nominator_id: nominator_id.clone(),
			amount,
		});
		Ok(())
	}

	/// Delegates to `withdraw_reward_at_index` with the current nonce.
	pub fn withdraw_reward(
		currency_id: T::CurrencyId,
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> Result<<SignedFixedPoint<T> as FixedPointNumber>::Inner, DispatchError> {
		let nonce = Self::nonce(vault_id);
		Self::withdraw_reward_at_index(nonce, currency_id, vault_id, nominator_id)
	}

	/// Withdraw all rewards earned by `vault_id` for the `nominator_id`.
	pub fn withdraw_reward_at_index(
		nonce: T::Nonce,
		currency_id: T::CurrencyId,
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> Result<<SignedFixedPoint<T> as FixedPointNumber>::Inner, DispatchError> {
		Self::apply_slash(vault_id, nominator_id)?;

		let reward = Self::compute_reward_at_index(nonce, currency_id, vault_id, nominator_id)?;
		let reward_as_fixed = SignedFixedPoint::<T>::checked_from_integer(reward)
			.ok_or(Error::<T>::TryIntoIntError)?;
		checked_sub_mut!(TotalRewards<T>, currency_id, (nonce, vault_id), &reward_as_fixed);

		let stake = Self::stake_at_index(nonce, vault_id, nominator_id);
		let reward_per_token = Self::reward_per_token(currency_id, (nonce, vault_id));
		<RewardTally<T>>::insert(
			currency_id,
			(nonce, vault_id, nominator_id),
			stake.checked_mul(&reward_per_token).ok_or(ArithmeticError::Overflow)?,
		);
		Self::deposit_event(Event::<T>::WithdrawReward {
			nonce,
			currency_id,
			vault_id: vault_id.clone(),
			nominator_id: nominator_id.clone(),
			amount: reward_as_fixed,
		});
		Ok(reward)
	}

	/// Force refund the entire nomination to `vault_id` by depositing it as reward. It
	/// returns the amount of collateral that is refunded
	pub fn force_refund(
		vault_id: &DefaultVaultId<T>,
	) -> Result<<SignedFixedPoint<T> as FixedPointNumber>::Inner, DispatchError> {
		let nonce = Self::nonce(vault_id);
		let total_current_stake = Self::total_current_stake_at_index(nonce, vault_id);

		// only withdraw the vault's stake from the current pool
		// nominators must withdraw manually using the nonce
		let stake = SignedFixedPoint::<T>::checked_from_integer(Self::compute_stake_at_index(
			nonce,
			vault_id,
			&vault_id.account_id,
		)?)
		.ok_or(Error::<T>::TryIntoIntError)?;
		Self::withdraw_stake(vault_id, &vault_id.account_id, stake, Some(nonce))?;
		Self::increment_nonce(vault_id)?;

		// only deposit vault stake after increasing the nonce
		Self::deposit_stake(vault_id, &vault_id.account_id, stake)?;
		Self::deposit_event(Event::<T>::ForceRefund { vault_id: vault_id.clone() });

		let refunded_collateral = total_current_stake
			.checked_sub(&stake)
			.ok_or(ArithmeticError::Underflow)?
			.truncate_to_inner()
			.ok_or(Error::<T>::TryIntoIntError)?;

		Ok(refunded_collateral)
	}

	pub fn increment_nonce(vault_id: &DefaultVaultId<T>) -> DispatchResult {
		<Nonce<T>>::mutate(vault_id, |nonce| {
			*nonce = nonce.checked_add(&T::Nonce::one()).ok_or(ArithmeticError::Overflow)?;
			Ok::<_, DispatchError>(())
		})?;
		Self::deposit_event(Event::<T>::IncreaseNonce {
			vault_id: vault_id.clone(),
			new_nonce: Self::nonce(vault_id),
		});
		Ok(())
	}

	pub fn add_reward_currency(currency_id: T::CurrencyId) -> DispatchResult {
		RewardCurrencies::<T>::try_mutate(|reward_currencies| {
			reward_currencies
				.try_insert(currency_id)
				.map_err(|_| Error::<T>::MaxRewardCurrencies)
		})?;
		Ok(())
	}

	pub fn get_all_reward_currencies() -> Result<Vec<T::CurrencyId>, DispatchError> {
		let mut values = RewardCurrencies::<T>::get().into_iter().collect::<Vec<_>>();
		values.push(T::GetNativeCurrencyId::get());
		Ok(values)
	}
}

pub trait Staking<VaultId, NominatorId, Index, Balance, CurrencyId> {
	/// Get the newest nonce for the staking pool.
	fn nonce(vault_id: &VaultId) -> Index;

	/// Deposit an `amount` of stake to the `vault_id` for the `nominator_id`.
	fn deposit_stake(
		vault_id: &VaultId,
		nominator_id: &NominatorId,
		amount: Balance,
	) -> Result<(), DispatchError>;

	/// Slash an `amount` of stake from the `vault_id`.
	fn slash_stake(vault_id: &VaultId, amount: Balance) -> Result<(), DispatchError>;

	/// Compute the stake in `vault_id` owned by `nominator_id`.
	fn compute_stake(
		vault_id: &VaultId,
		nominator_id: &NominatorId,
	) -> Result<Balance, DispatchError>;

	/// Compute the total stake in `vault_id` **after** slashing.
	fn total_stake(vault_id: &VaultId) -> Result<Balance, DispatchError>;

	/// Distribute the `reward` to all participants.
	fn distribute_reward(
		vault_id: &VaultId,
		reward: Balance,
		currency_id: CurrencyId,
	) -> Result<Balance, DispatchError>;

	/// Compute the expected reward for `nominator_id` who is nominating `vault_id`.
	fn compute_reward(
		vault_id: &VaultId,
		nominator_id: &NominatorId,
		currency_id: CurrencyId,
	) -> Result<Balance, DispatchError>;

	/// Withdraw an `amount` of stake from the `vault_id` for the `nominator_id`.
	fn withdraw_stake(
		vault_id: &VaultId,
		nominator_id: &NominatorId,
		amount: Balance,
		index: Option<Index>,
	) -> DispatchResult;

	/// Withdraw all rewards earned by `vault_id` for the `nominator_id`.
	fn withdraw_reward(
		vault_id: &VaultId,
		nominator_id: &NominatorId,
		index: Option<Index>,
		currency_id: CurrencyId,
	) -> Result<Balance, DispatchError>;

	/// Force refund the entire nomination to `vault_id`.
	fn force_refund(vault_id: &VaultId) -> Result<Balance, DispatchError>;

	// set a new reward currency

	fn add_reward_currency(currency: CurrencyId) -> Result<(), DispatchError>;

	fn get_all_reward_currencies() -> Result<Vec<CurrencyId>, DispatchError>;
}

impl<T, Balance> Staking<DefaultVaultId<T>, T::AccountId, T::Nonce, Balance, T::CurrencyId>
	for Pallet<T>
where
	T: Config,

	Balance: BalanceToFixedPoint<SignedFixedPoint<T>>,
	<T::SignedFixedPoint as FixedPointNumber>::Inner: TryInto<Balance>,
{
	fn nonce(vault_id: &DefaultVaultId<T>) -> T::Nonce {
		Pallet::<T>::nonce(vault_id)
	}

	fn deposit_stake(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		amount: Balance,
	) -> DispatchResult {
		Pallet::<T>::deposit_stake(
			vault_id,
			nominator_id,
			amount.to_fixed().ok_or(Error::<T>::TryIntoIntError)?,
		)
	}

	fn slash_stake(vault_id: &DefaultVaultId<T>, amount: Balance) -> DispatchResult {
		Pallet::<T>::slash_stake(vault_id, amount.to_fixed().ok_or(Error::<T>::TryIntoIntError)?)
	}

	fn compute_stake(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> Result<Balance, DispatchError> {
		Pallet::<T>::compute_stake(vault_id, nominator_id)?
			.try_into()
			.map_err(|_| Error::<T>::TryIntoIntError.into())
	}

	fn total_stake(vault_id: &DefaultVaultId<T>) -> Result<Balance, DispatchError> {
		Pallet::<T>::total_current_stake(vault_id)?
			.try_into()
			.map_err(|_| Error::<T>::TryIntoIntError.into())
	}

	fn distribute_reward(
		vault_id: &DefaultVaultId<T>,
		amount: Balance,
		currency_id: T::CurrencyId,
	) -> Result<Balance, DispatchError> {
		Pallet::<T>::distribute_reward(
			currency_id,
			vault_id,
			amount.to_fixed().ok_or(Error::<T>::TryIntoIntError)?,
		)?
		.truncate_to_inner()
		.ok_or(Error::<T>::TryIntoIntError)?
		.try_into()
		.map_err(|_| Error::<T>::TryIntoIntError.into())
	}

	fn compute_reward(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		currency_id: T::CurrencyId,
	) -> Result<Balance, DispatchError> {
		Pallet::<T>::compute_reward(currency_id, vault_id, nominator_id)?
			.try_into()
			.map_err(|_| Error::<T>::TryIntoIntError.into())
	}

	fn withdraw_stake(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		amount: Balance,
		index: Option<T::Nonce>,
	) -> DispatchResult {
		Pallet::<T>::withdraw_stake(
			vault_id,
			nominator_id,
			amount.to_fixed().ok_or(Error::<T>::TryIntoIntError)?,
			index,
		)
	}

	fn withdraw_reward(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		index: Option<T::Nonce>,
		currency_id: T::CurrencyId,
	) -> Result<Balance, DispatchError> {
		let nonce = index.unwrap_or_else(|| Pallet::<T>::nonce(vault_id));
		Pallet::<T>::withdraw_reward_at_index(nonce, currency_id, vault_id, nominator_id)?
			.try_into()
			.map_err(|_| Error::<T>::TryIntoIntError.into())
	}

	fn force_refund(vault_id: &DefaultVaultId<T>) -> Result<Balance, DispatchError> {
		Pallet::<T>::force_refund(vault_id)?
			.try_into()
			.map_err(|_| Error::<T>::TryIntoIntError.into())
	}

	fn add_reward_currency(currency: T::CurrencyId) -> Result<(), DispatchError> {
		Pallet::<T>::add_reward_currency(currency)
	}

	fn get_all_reward_currencies() -> Result<Vec<T::CurrencyId>, DispatchError> {
		Pallet::<T>::get_all_reward_currencies()
	}
}

pub mod migration {
	use frame_support::transactional;
	use orml_traits::MultiCurrency;

	use super::*;

	#[cfg(test)]
	mod tests {
		use frame_support::assert_ok;
		use sp_arithmetic::FixedI128;

		use super::*;

		/// The code as implemented befor the fix
		fn legacy_withdraw_reward_at_index<T: Config>(
			nonce: T::Nonce,
			currency_id: T::CurrencyId,
			vault_id: &DefaultVaultId<T>,
			nominator_id: &T::AccountId,
		) -> Result<<SignedFixedPoint<T> as FixedPointNumber>::Inner, DispatchError> {
			let reward =
				Pallet::<T>::compute_reward_at_index(nonce, currency_id, vault_id, nominator_id)?;
			let reward_as_fixed = SignedFixedPoint::<T>::checked_from_integer(reward)
				.ok_or(Error::<T>::TryIntoIntError)?;
			checked_sub_mut!(TotalRewards<T>, currency_id, (nonce, vault_id), &reward_as_fixed);

			let stake = Pallet::<T>::stake_at_index(nonce, vault_id, nominator_id);
			let reward_per_token = Pallet::<T>::reward_per_token(currency_id, (nonce, vault_id));
			<RewardTally<T>>::insert(
				currency_id,
				(nonce, vault_id, nominator_id),
				stake.checked_mul(&reward_per_token).ok_or(ArithmeticError::Overflow)?,
			);
			Pallet::<T>::deposit_event(Event::<T>::WithdrawReward {
				nonce,
				currency_id,
				vault_id: vault_id.clone(),
				nominator_id: nominator_id.clone(),
				amount: reward_as_fixed,
			});
			Ok(reward)
		}

		fn setup_broken_state() {
			use mock::*;
			// without the `apply_slash` in withdraw_rewards, the following sequence fails in the
			// last step: [distribute_reward, slash_stake, withdraw_reward, distribute_reward,
			// withdraw_reward]

			// step 1: initial (normal) flow
			assert_ok!(Staking::add_reward_currency(DEFAULT_WRAPPED_CURRENCY));
			assert_ok!(Staking::deposit_stake(&VAULT, &VAULT.account_id, fixed!(50)));
			assert_ok!(Staking::distribute_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, fixed!(10000)));
			assert_ok!(
				Staking::compute_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
				10000
			);

			// step 2: slash
			assert_ok!(Staking::slash_stake(&VAULT, fixed!(30)));
			assert_ok!(Staking::compute_stake(&VAULT, &VAULT.account_id), 20);

			// step 3: withdraw rewards
			assert_ok!(
				Staking::compute_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
				10000
			);
			assert_ok!(
				legacy_withdraw_reward_at_index::<Test>(
					0,
					DEFAULT_WRAPPED_CURRENCY,
					&VAULT,
					&VAULT.account_id
				),
				10000
			);

			assert_ok!(Staking::distribute_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, fixed!(10000)));
			assert_ok!(
				legacy_withdraw_reward_at_index::<Test>(
					0,
					DEFAULT_WRAPPED_CURRENCY,
					&VAULT,
					&VAULT.account_id
				),
				0
			);
			// check that we keep track of the tokens we're still owed
			assert_total_rewards(10000);

			assert_ok!(Staking::distribute_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, fixed!(2000)));
			assert_eq!(
				Staking::total_rewards(DEFAULT_WRAPPED_CURRENCY, (0, VAULT.clone())),
				FixedI128::from(12000)
			);
			assert_ok!(
				legacy_withdraw_reward_at_index::<Test>(
					0,
					DEFAULT_WRAPPED_CURRENCY,
					&VAULT,
					&VAULT.account_id
				),
				0
			);
			assert_total_rewards(12000);
		}

		fn assert_total_rewards(amount: i128) {
			assert_ok!(Staking::add_reward_currency(DEFAULT_WRAPPED_CURRENCY));
			use mock::*;
			assert_eq!(
				Staking::total_rewards(DEFAULT_WRAPPED_CURRENCY, (0, VAULT.clone())),
				FixedI128::from(amount)
			);
		}

		#[test]
		fn test_total_rewards_tracking_in_buggy_code() {
			use mock::*;
			run_test(|| {
				assert_ok!(Staking::add_reward_currency(DEFAULT_WRAPPED_CURRENCY));
				setup_broken_state();

				assert_total_rewards(12000);

				// now simulate that we deploy the fix, but don't the migration.
				assert_ok!(Staking::distribute_reward(
					DEFAULT_WRAPPED_CURRENCY,
					&VAULT,
					fixed!(3000)
				));
				assert_total_rewards(15000);

				// the first withdraw are still incorrect: we need a sequence [withdraw_reward,
				// distribute_reward] to start working correctly again
				assert_ok!(
					Staking::withdraw_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
					0
				);
				assert_ok!(
					Staking::withdraw_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
					0
				);
				assert_total_rewards(15000);

				// distribute 500 more - we should actually receive that now.
				assert_ok!(Staking::distribute_reward(
					DEFAULT_WRAPPED_CURRENCY,
					&VAULT,
					fixed!(500)
				));
				assert_total_rewards(15500);
				assert_ok!(
					Staking::withdraw_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
					500
				);
				assert_total_rewards(15000);
			})
		}

		#[test]
		fn test_migration() {
			use mock::*;
			run_test(|| {
				assert_ok!(Staking::add_reward_currency(DEFAULT_WRAPPED_CURRENCY));
				let fee_pool_account_id = 23;

				assert_ok!(<orml_tokens::Pallet<Test> as MultiCurrency<
					<Test as frame_system::Config>::AccountId,
				>>::deposit(DEFAULT_WRAPPED_CURRENCY, &fee_pool_account_id, 100_000,));

				setup_broken_state();
				assert_total_rewards(12000);

				// now simulate that we deploy the fix and do the migration

				assert_ok!(fix_broken_state::<Test, _>(fee_pool_account_id));
				assert_total_rewards(0);
				assert_eq!(
					<orml_tokens::Pallet<Test> as MultiCurrency<
						<Test as frame_system::Config>::AccountId,
					>>::free_balance(DEFAULT_WRAPPED_CURRENCY, &VAULT.account_id),
					12000
				);

				// check that we can't withdraw any additional amount
				assert_ok!(
					Staking::withdraw_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
					0
				);

				// check that behavior is back to normal
				assert_ok!(Staking::distribute_reward(
					DEFAULT_WRAPPED_CURRENCY,
					&VAULT,
					fixed!(3000)
				));
				assert_total_rewards(3000);
				assert_ok!(
					Staking::withdraw_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
					3000
				);
				assert_total_rewards(0);
			});
		}

		/// like setup_broken_state, but after depositing such a large sum that you can withdraw
		/// despite the slash bug (it will withdraw an incorrect but non-zero amount)
		fn setup_broken_state_with_withdrawable_reward() {
			use mock::*;
			assert_ok!(Staking::add_reward_currency(DEFAULT_WRAPPED_CURRENCY));
			setup_broken_state();
			assert_total_rewards(12000);
			assert_ok!(Staking::distribute_reward(
				DEFAULT_WRAPPED_CURRENCY,
				&VAULT,
				fixed!(1_000_000)
			));
			assert_total_rewards(1_012_000);
		}

		#[test]
		fn test_broken_state_with_withdrawable_amount() {
			use mock::*;
			run_test(|| {
				assert_ok!(Staking::add_reward_currency(DEFAULT_WRAPPED_CURRENCY));
				setup_broken_state_with_withdrawable_reward();
				assert_total_rewards(1_012_000);

				// check that we can indeed withdraw some non-zero amount
				assert_ok!(
					Staking::withdraw_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
					967_000
				);
				assert_total_rewards(1_012_000 - 967_000);

				assert_ok!(
					Staking::withdraw_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
					0
				);
			});
		}

		#[test]
		fn test_migration_of_account_with_withdrawable_amount() {
			use mock::*;
			run_test(|| {
				assert_ok!(Staking::add_reward_currency(DEFAULT_WRAPPED_CURRENCY));
				let fee_pool_account_id = 23;

				assert_ok!(<orml_tokens::Pallet<Test> as MultiCurrency<
					<Test as frame_system::Config>::AccountId,
				>>::deposit(DEFAULT_WRAPPED_CURRENCY, &fee_pool_account_id, 10_000_000,));

				setup_broken_state_with_withdrawable_reward();
				assert_total_rewards(1_012_000);

				// now simulate that we deploy the fix and do the migration

				assert_ok!(fix_broken_state::<Test, _>(fee_pool_account_id));
				assert_total_rewards(0);
				assert_eq!(
					<orml_tokens::Pallet<Test> as MultiCurrency<
						<Test as frame_system::Config>::AccountId,
					>>::free_balance(DEFAULT_WRAPPED_CURRENCY, &VAULT.account_id),
					1_012_000
				);

				// check that we can't withdraw any additional amount
				assert_ok!(
					Staking::withdraw_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
					0
				);

				// check that behavior is back to normal
				assert_ok!(Staking::distribute_reward(
					DEFAULT_WRAPPED_CURRENCY,
					&VAULT,
					fixed!(3000)
				));
				assert_total_rewards(3000);
				assert_ok!(
					Staking::withdraw_reward(DEFAULT_WRAPPED_CURRENCY, &VAULT, &VAULT.account_id),
					3000
				);
				assert_total_rewards(0);
			});
		}
	}

	#[transactional]
	pub fn fix_broken_state<T, U>(fee_pool_account_id: T::AccountId) -> DispatchResult
	where
		T: Config<SignedInner = U> + orml_tokens::Config<CurrencyId = <T as Config>::CurrencyId>,
		U: TryInto<<T as orml_tokens::Config>::Balance>,
	{
		use sp_std::vec::Vec;

		// first collect to a vec - for safety we won't modify this map while we iterate over it
		let total_rewards: Vec<_> = TotalRewards::<T>::drain().collect();

		for (currency_id, (_idx, vault_id), value) in total_rewards {
			let missed_reward = value
				.truncate_to_inner()
				.ok_or(Error::<T>::TryIntoIntError)?
				.try_into()
				.map_err(|_| Error::<T>::TryIntoIntError)?;

			<orml_tokens::Pallet<T> as MultiCurrency<T::AccountId>>::transfer(
				currency_id,
				&fee_pool_account_id,
				&vault_id.account_id,
				missed_reward,
			)?;

			Pallet::<T>::withdraw_reward(currency_id, &vault_id, &vault_id.account_id)?;
		}

		// an additional drain is required to pass the
		// `test_migration_of_account_with_withdrawable_amount` test - otherwise TotalRewards are
		// set to zero
		let _: Vec<_> = TotalRewards::<T>::drain().collect();

		Ok(())
	}
}
