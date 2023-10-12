#[cfg(test)]
use mocktopus::macros::mockable;

#[cfg_attr(test, mockable)]
pub(crate) mod currency {
	use currency::Amount;

	use crate::types::CurrencyId;

	pub fn get_free_balance<T: crate::Config>(
		currency_id: CurrencyId<T>,
		id: &T::AccountId,
	) -> Amount<T> {
		currency::get_free_balance::<T>(currency_id, id)
	}

	pub fn get_reserved_balance<T: crate::Config>(
		currency_id: CurrencyId<T>,
		id: &T::AccountId,
	) -> Amount<T> {
		currency::get_reserved_balance::<T>(currency_id, id)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod security {
	pub fn active_block_number<T: crate::Config>() -> T::BlockNumber {
		<security::Pallet<T>>::active_block_number()
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod staking {
	use frame_support::dispatch::{DispatchError, DispatchResult};

	use currency::Amount;
	use staking::Staking;

	use crate::{types::BalanceOf, DefaultVaultId};

	pub fn deposit_stake<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		amount: &Amount<T>,
	) -> DispatchResult {
		T::VaultStaking::deposit_stake(vault_id, nominator_id, amount.amount())
	}

	pub fn withdraw_stake<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		amount: &Amount<T>,
	) -> DispatchResult {
		T::VaultStaking::withdraw_stake(vault_id, nominator_id, amount.amount(), None)
	}

	pub fn slash_stake<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> DispatchResult {
		T::VaultStaking::slash_stake(vault_id, amount.amount())
	}

	pub fn compute_stake<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
	) -> Result<BalanceOf<T>, DispatchError> {
		T::VaultStaking::compute_stake(vault_id, nominator_id)
	}

	pub fn total_current_stake<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<BalanceOf<T>, DispatchError> {
		T::VaultStaking::total_stake(vault_id)
	}

	pub fn total_current_stake_as_amount<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<Amount<T>, DispatchError> {
		let vault_total_stake = T::VaultStaking::total_stake(vault_id)?;
		Ok(Amount::<T>::new(vault_total_stake, vault_id.collateral_currency()))
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod pooled_rewards {

	use currency::Amount;
	use frame_support::dispatch::DispatchError;
	use pooled_rewards::RewardsApi;

	use crate::DefaultVaultId;

	pub fn set_stake<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> Result<(), DispatchError> {
		T::VaultRewards::set_stake(&vault_id.collateral_currency(), &vault_id, amount.amount())
	}

	#[cfg(feature = "integration-tests")]
	pub fn get_stake<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<crate::BalanceOf<T>, DispatchError> {
		T::VaultRewards::get_stake(&vault_id.collateral_currency(), &vault_id)
	}
}
