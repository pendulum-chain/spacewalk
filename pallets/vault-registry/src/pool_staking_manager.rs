use crate::*;
pub struct PoolManager<T>(PhantomData<T>);

impl<T: Config> PoolManager<T> {
	pub fn deposit_collateral(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		amount: &Amount<T>,
	) -> Result<(), DispatchError> {
		ext::reward_distribution::withdraw_all_rewards_from_vault::<T>(&vault_id)?;
		ext::staking::deposit_stake::<T>(vault_id, nominator_id, amount)?;

		// also propagate to reward & capacity pools
		Self::update_reward_stake(vault_id)
	}

	pub fn withdraw_collateral(
		vault_id: &DefaultVaultId<T>,
		nominator_id: &T::AccountId,
		amount: &Amount<T>,
		nonce: Option<T::Nonce>,
	) -> Result<(), DispatchError> {
		ext::reward_distribution::withdraw_all_rewards_from_vault::<T>(&vault_id)?;
		ext::staking::withdraw_stake::<T>(vault_id, nominator_id, amount, nonce)?;

		// also propagate to reward & capacity pools
		Self::update_reward_stake(vault_id)?;

		Ok(())
	}

	pub fn slash_collateral(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> Result<(), DispatchError> {
		ext::reward_distribution::withdraw_all_rewards_from_vault::<T>(&vault_id)?;
		ext::staking::slash_stake(vault_id, amount)?;

		// also propagate to reward & capacity pools
		Self::update_reward_stake(vault_id)
	}

	pub fn kick_nominators(vault_id: &DefaultVaultId<T>) -> Result<BalanceOf<T>, DispatchError> {
		ext::reward_distribution::withdraw_all_rewards_from_vault::<T>(&vault_id)?;
		let ret = ext::staking::force_refund::<T>(vault_id)?;

		Self::update_reward_stake(vault_id)?;

		Ok(ret)
	}

	pub fn on_vault_settings_change(vault_id: &DefaultVaultId<T>) -> Result<(), DispatchError> {
		ext::reward_distribution::withdraw_all_rewards_from_vault::<T>(&vault_id)?;
		Self::update_reward_stake(vault_id)
	}

	pub(crate) fn update_reward_stake(vault_id: &DefaultVaultId<T>) -> Result<(), DispatchError> {
		let vault = Pallet::<T>::get_vault_from_id(vault_id)?;
		let new_reward_stake: currency::Amount<T> = if !vault.accepts_new_issues() {
			// if the vault is not accepting new issues it's not getting rewards
			Amount::zero(vault_id.collateral_currency())
		} else {
			let total_stake = ext::staking::total_current_stake::<T>(vault_id)?;
			Amount::new(total_stake, vault_id.collateral_currency())
		};

		ext::pooled_rewards::set_stake(vault_id, &new_reward_stake)?;

		Ok(())
	}
}
