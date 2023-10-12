//! # Fee Module

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
extern crate mocktopus;

use codec::EncodeLike;
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	traits::Get,
	transactional, PalletId,
};
#[cfg(test)]
use mocktopus::macros::mockable;
use sp_arithmetic::{traits::*, FixedPointNumber, FixedPointOperand, Perquintill};
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned};
use sp_std::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
};

use currency::{Amount, CurrencyId, OnSweep};
pub use default_weights::{SubstrateWeight, WeightInfo};
pub use pallet::*;
pub use pooled_rewards::RewardsApi;
use types::{BalanceOf, DefaultVaultId, SignedFixedPoint, UnsignedFixedPoint};

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod default_weights;
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod types;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use super::*;

	/// ## Configuration
	/// The pallet's configuration trait.
	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ security::Config
		+ currency::Config<
			UnsignedFixedPoint = UnsignedFixedPoint<Self>,
			SignedFixedPoint = SignedFixedPoint<Self>,
		>
	{
		/// The fee module id, used for deriving its sovereign account ID.
		#[pallet::constant]
		type FeePalletId: Get<PalletId>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// Signed fixed point type.
		type SignedFixedPoint: FixedPointNumber<Inner = <Self as Config>::SignedInner>
			+ Encode
			+ EncodeLike
			+ Decode;

		/// The `Inner` type of the `SignedFixedPoint`.
		type SignedInner: Debug
			+ CheckedDiv
			+ TryFrom<BalanceOf<Self>>
			+ TryInto<BalanceOf<Self>>
			+ MaybeSerializeDeserialize;

		/// Unsigned fixed point type.
		type UnsignedFixedPoint: FixedPointNumber<Inner = <Self as Config>::UnsignedInner>
			+ Encode
			+ EncodeLike
			+ Decode
			+ MaybeSerializeDeserialize
			+ TypeInfo;

		/// The `Inner` type of the `UnsignedFixedPoint`.
		type UnsignedInner: Debug
			+ One
			+ CheckedMul
			+ CheckedDiv
			+ FixedPointOperand
			+ AtLeast32BitUnsigned
			+ Default
			+ Encode
			+ EncodeLike
			+ Decode
			+ MaybeSerializeDeserialize
			+ TypeInfo;

		/// Vault reward pool.
		type VaultRewards: pooled_rewards::RewardsApi<
			CurrencyId<Self>,
			DefaultVaultId<Self>,
			BalanceOf<Self>,
			CurrencyId<Self>,
		>;

		/// Vault staking pool.
		type VaultStaking: staking::Staking<
			DefaultVaultId<Self>,
			Self::AccountId,
			Self::Index,
			BalanceOf<Self>,
			Self::CurrencyId,
		>;

		/// Handler to transfer undistributed rewards.
		type OnSweep: OnSweep<Self::AccountId, Amount<Self>>;

		//currency to usd interface
		type BaseCurrency: Get<CurrencyId<Self>>;

		/// Maximum expected value to set the storage fields to.
		#[pallet::constant]
		type MaxExpectedValue: Get<UnsignedFixedPoint<Self>>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Unable to convert value.
		TryIntoIntError,
		/// Value exceeds the expected upper bound for storage fields in this pallet.
		AboveMaxExpectedValue,
		Overflow,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// # Issue

	/// Fee share that users need to pay to issue tokens.
	#[pallet::storage]
	#[pallet::getter(fn issue_fee)]
	pub type IssueFee<T: Config> = StorageValue<_, UnsignedFixedPoint<T>, ValueQuery>;

	/// Default griefing collateral (e.g. DOT/KSM) as a percentage of the locked
	/// collateral of a Vault a user has to lock to issue tokens.
	#[pallet::storage]
	#[pallet::getter(fn issue_griefing_collateral)]
	pub type IssueGriefingCollateral<T: Config> =
		StorageValue<_, UnsignedFixedPoint<T>, ValueQuery>;

	/// # Redeem

	/// Fee share that users need to pay to redeem tokens.
	#[pallet::storage]
	#[pallet::getter(fn redeem_fee)]
	pub type RedeemFee<T: Config> = StorageValue<_, UnsignedFixedPoint<T>, ValueQuery>;

	/// # Vault Registry

	/// If users execute a redeem with a Vault flagged for premium redeem,
	/// they can earn a collateral premium, slashed from the Vault.
	#[pallet::storage]
	#[pallet::getter(fn premium_redeem_fee)]
	pub type PremiumRedeemFee<T: Config> = StorageValue<_, UnsignedFixedPoint<T>, ValueQuery>;

	/// Fee that a Vault has to pay if it fails to execute redeem or replace requests
	/// (for redeem, on top of the slashed value of the request). The fee is
	/// paid in collateral based on the token amount at the current exchange rate.
	#[pallet::storage]
	#[pallet::getter(fn punishment_fee)]
	pub type PunishmentFee<T: Config> = StorageValue<_, UnsignedFixedPoint<T>, ValueQuery>;

	/// # Replace

	/// Default griefing collateral (e.g. DOT/KSM) as a percentage of the to-be-locked collateral
	/// of the new Vault. This collateral will be slashed and allocated to the replacing Vault
	/// if the to-be-replaced Vault does not transfer the Stellar assets on time.
	#[pallet::storage]
	#[pallet::getter(fn replace_griefing_collateral)]
	pub type ReplaceGriefingCollateral<T: Config> =
		StorageValue<_, UnsignedFixedPoint<T>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub issue_fee: UnsignedFixedPoint<T>,
		pub issue_griefing_collateral: UnsignedFixedPoint<T>,
		pub redeem_fee: UnsignedFixedPoint<T>,
		pub premium_redeem_fee: UnsignedFixedPoint<T>,
		pub punishment_fee: UnsignedFixedPoint<T>,
		pub replace_griefing_collateral: UnsignedFixedPoint<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				issue_fee: Default::default(),
				issue_griefing_collateral: Default::default(),
				redeem_fee: Default::default(),
				premium_redeem_fee: Default::default(),
				punishment_fee: Default::default(),
				replace_griefing_collateral: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			IssueFee::<T>::put(self.issue_fee);
			IssueGriefingCollateral::<T>::put(self.issue_griefing_collateral);
			RedeemFee::<T>::put(self.redeem_fee);
			PremiumRedeemFee::<T>::put(self.premium_redeem_fee);
			PunishmentFee::<T>::put(self.punishment_fee);
			ReplaceGriefingCollateral::<T>::put(self.replace_griefing_collateral);
		}
	}

	#[pallet::pallet]
	#[pallet::without_storage_info] // fixedpoint does not yet implement MaxEncodedLen
	pub struct Pallet<T>(_);

	// The pallet's dispatchable functions.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Withdraw all rewards from the `origin` account in the `vault_id` staking pool.
		///
		/// # Arguments
		///
		/// * `origin` - signing account
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::withdraw_rewards())]
		#[transactional]
		pub fn withdraw_rewards(
			origin: OriginFor<T>,
			vault_id: DefaultVaultId<T>,
			index: Option<T::Index>,
		) -> DispatchResultWithPostInfo {
			let nominator_id = ensure_signed(origin)?;
			Self::withdraw_from_reward_pool::<T::VaultRewards, T::VaultStaking>(
				&vault_id,
				&nominator_id,
				index,
			)?;
			Ok(().into())
		}

		/// Changes the issue fee percentage (only executable by the Root account)
		///
		/// # Arguments
		///
		/// * `origin` - signing account
		/// * `fee` - the new fee
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_issue_fee())]
		#[transactional]
		pub fn set_issue_fee(
			origin: OriginFor<T>,
			fee: UnsignedFixedPoint<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			ensure!(fee <= Self::get_max_expected_value(), Error::<T>::AboveMaxExpectedValue);
			IssueFee::<T>::put(fee);
			Ok(().into())
		}

		/// Changes the issue griefing collateral percentage (only executable by the Root account)
		///
		/// # Arguments
		///
		/// * `origin` - signing account
		/// * `griefing_collateral` - the new griefing collateral
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::set_issue_griefing_collateral())]
		#[transactional]
		pub fn set_issue_griefing_collateral(
			origin: OriginFor<T>,
			griefing_collateral: UnsignedFixedPoint<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			ensure!(
				griefing_collateral <= Self::get_max_expected_value(),
				Error::<T>::AboveMaxExpectedValue
			);
			IssueGriefingCollateral::<T>::put(griefing_collateral);
			Ok(().into())
		}

		/// Changes the redeem fee percentage (only executable by the Root account)
		///
		/// # Arguments
		///
		/// * `origin` - signing account
		/// * `fee` - the new fee
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::set_redeem_fee())]
		#[transactional]
		pub fn set_redeem_fee(
			origin: OriginFor<T>,
			fee: UnsignedFixedPoint<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			ensure!(fee <= Self::get_max_expected_value(), Error::<T>::AboveMaxExpectedValue);
			RedeemFee::<T>::put(fee);
			Ok(().into())
		}

		/// Changes the premium redeem fee percentage (only executable by the Root account)
		///
		/// # Arguments
		///
		/// * `origin` - signing account
		/// * `fee` - the new fee
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::set_premium_redeem_fee())]
		#[transactional]
		pub fn set_premium_redeem_fee(
			origin: OriginFor<T>,
			fee: UnsignedFixedPoint<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			ensure!(fee <= Self::get_max_expected_value(), Error::<T>::AboveMaxExpectedValue);
			PremiumRedeemFee::<T>::put(fee);
			Ok(().into())
		}

		/// Changes the punishment fee percentage (only executable by the Root account)
		///
		/// # Arguments
		///
		/// * `origin` - signing account
		/// * `fee` - the new fee
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::set_punishment_fee())]
		#[transactional]
		pub fn set_punishment_fee(
			origin: OriginFor<T>,
			fee: UnsignedFixedPoint<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			ensure!(fee <= Self::get_max_expected_value(), Error::<T>::AboveMaxExpectedValue);
			PunishmentFee::<T>::put(fee);
			Ok(().into())
		}

		/// Changes the replace griefing collateral percentage (only executable by the Root account)
		///
		/// # Arguments
		///
		/// * `origin` - signing account
		/// * `griefing_collateral` - the new griefing collateral
		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::set_replace_griefing_collateral())]
		#[transactional]
		pub fn set_replace_griefing_collateral(
			origin: OriginFor<T>,
			griefing_collateral: UnsignedFixedPoint<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			ensure!(
				griefing_collateral <= Self::get_max_expected_value(),
				Error::<T>::AboveMaxExpectedValue
			);
			ReplaceGriefingCollateral::<T>::put(griefing_collateral);
			Ok(().into())
		}
	}
}

// "Internal" functions, callable by code.
#[cfg_attr(test, mockable)]
impl<T: Config> Pallet<T> {
	/// The account ID of the fee pool.
	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub fn fee_pool_account_id() -> T::AccountId {
		<T as Config>::FeePalletId::get().into_account_truncating()
	}

	pub fn get_max_expected_value() -> UnsignedFixedPoint<T> {
		<T as Config>::MaxExpectedValue::get()
	}

	// Public functions exposed to other pallets

	/// Distribute rewards to participants.
	///
	/// # Arguments
	///
	/// * `amount` - amount of rewards
	pub fn distribute_rewards(amount: &Amount<T>) -> DispatchResult {
		// distribute vault rewards and return leftover
		let remaining = Self::distribute(amount)?;
		if !remaining.is_zero() {
			// sweep the remaining rewards to the treasury if non-zero
			T::OnSweep::on_sweep(&Self::fee_pool_account_id(), remaining)?;
		}
		Ok(())
	}

	/// Calculate the required issue fee in tokens.
	///
	/// # Arguments
	///
	/// * `amount` - issue amount in tokens
	pub fn get_issue_fee(amount: &Amount<T>) -> Result<Amount<T>, DispatchError> {
		amount.rounded_mul(<IssueFee<T>>::get())
	}

	/// Calculate the required issue griefing collateral.
	///
	/// # Arguments
	///
	/// * `amount` - issue amount in collateral (at current exchange rate)
	pub fn get_issue_griefing_collateral(amount: &Amount<T>) -> Result<Amount<T>, DispatchError> {
		amount.rounded_mul(<IssueGriefingCollateral<T>>::get())
	}

	/// Calculate the required redeem fee in tokens. Upon execution, the
	/// rewards should be forwarded to the fee pool instead of being burned.
	///
	/// # Arguments
	///
	/// * `amount` - redeem amount in tokens
	pub fn get_redeem_fee(amount: &Amount<T>) -> Result<Amount<T>, DispatchError> {
		amount.rounded_mul(<RedeemFee<T>>::get())
	}

	/// Calculate the premium redeem fee in collateral for a user to get if redeeming
	/// with a Vault below the premium redeem threshold.
	///
	/// # Arguments
	///
	/// * `amount` - amount in collateral (at current exchange rate)
	pub fn get_premium_redeem_fee(amount: &Amount<T>) -> Result<Amount<T>, DispatchError> {
		amount.rounded_mul(<PremiumRedeemFee<T>>::get())
	}

	/// Calculate punishment fee for a Vault that fails to execute a redeem
	/// request before the expiry.
	///
	/// # Arguments
	///
	/// * `amount` - amount in collateral (at current exchange rate)
	pub fn get_punishment_fee(amount: &Amount<T>) -> Result<Amount<T>, DispatchError> {
		amount.rounded_mul(<PunishmentFee<T>>::get())
	}

	/// Calculate the required replace griefing collateral.
	///
	/// # Arguments
	///
	/// * `amount` - replace amount in collateral (at current exchange rate)
	pub fn get_replace_griefing_collateral(amount: &Amount<T>) -> Result<Amount<T>, DispatchError> {
		amount.rounded_mul(<ReplaceGriefingCollateral<T>>::get())
	}

	pub fn withdraw_all_vault_rewards(vault_id: &DefaultVaultId<T>) -> DispatchResult {
		Self::distribute_from_reward_pool::<T::VaultRewards, T::VaultStaking>(vault_id)?;
		Ok(())
	}

	// Private functions internal to this pallet
	/// Withdraw rewards from a pool and transfer to `account_id`.
	fn withdraw_from_reward_pool<
		Rewards: pooled_rewards::RewardsApi<CurrencyId<T>, DefaultVaultId<T>, BalanceOf<T>, CurrencyId<T>>,
		Staking: staking::Staking<DefaultVaultId<T>, T::AccountId, T::Index, BalanceOf<T>, CurrencyId<T>>,
	>(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		index: Option<T::Index>,
	) -> DispatchResult {
		Self::distribute_from_reward_pool::<Rewards, Staking>(vault_id)?;

		for currency_id in [vault_id.wrapped_currency(), T::GetNativeCurrencyId::get()] {
			let rewards = Staking::withdraw_reward(vault_id, nominator_id, index, currency_id)?;
			let amount = Amount::<T>::new(rewards, currency_id);
			amount.transfer(&Self::fee_pool_account_id(), nominator_id)?;
		}
		Ok(())
	}

	fn distribute(reward: &Amount<T>) -> Result<Amount<T>, DispatchError> {
		//fetch total stake (all), and calulate total usd stake across pools
		//distribute the rewards into each reward pool for each collateral,
		//taking into account it's value in usd

		//TODO this logic will go to the reward-distribution pallet most likely

		let total_stakes = T::VaultRewards::get_total_stake_all_pools()?;
		let total_stake_in_usd = BalanceOf::<T>::default();
		for (currency_id, stake) in total_stakes.clone().into_iter() {
			let stake_in_amount = Amount::<T>::new(stake, currency_id);
			let stake_in_usd = stake_in_amount.convert_to(<T as Config>::BaseCurrency::get())?;
			total_stake_in_usd
				.checked_add(&stake_in_usd.amount())
				.ok_or(Error::<T>::Overflow)?;
		}

		let error_reward_accum = Amount::<T>::zero(reward.currency());

		for (currency_id, stake) in total_stakes.into_iter() {
			let stake_in_amount = Amount::<T>::new(stake, currency_id);
			let stake_in_usd = stake_in_amount.convert_to(<T as Config>::BaseCurrency::get())?;
			let percentage = Perquintill::from_rational(stake_in_usd.amount(), total_stake_in_usd);

			//TODO multiply with floor or ceil?
			let reward_for_pool = percentage.mul_floor(reward.amount());

			if T::VaultRewards::distribute_reward(&currency_id, reward.currency(), reward_for_pool)
				.is_err()
			{
				error_reward_accum.checked_add(&reward)?;
			}
		}
		Ok(error_reward_accum)
	}

	pub fn distribute_from_reward_pool<
		Rewards: pooled_rewards::RewardsApi<CurrencyId<T>, DefaultVaultId<T>, BalanceOf<T>, CurrencyId<T>>,
		Staking: staking::Staking<DefaultVaultId<T>, T::AccountId, T::Index, BalanceOf<T>, CurrencyId<T>>,
	>(
		vault_id: &DefaultVaultId<T>,
	) -> DispatchResult {
		for currency_id in [vault_id.wrapped_currency(), T::GetNativeCurrencyId::get()] {
			let reward =
				Rewards::withdraw_reward(&vault_id.collateral_currency(), &vault_id, currency_id)?;
			Staking::distribute_reward(vault_id, reward, currency_id)?;
		}

		Ok(())
	}
}
