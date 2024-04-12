use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_system::offchain::SubmitTransaction;
use sp_arithmetic::ArithmeticError;
use sp_arithmetic::traits::{CheckedDiv, CheckedSub, One};
use sp_core::{Get, U256};
use sp_runtime::DispatchError;
use sp_runtime::traits::AccountIdConversion;
use currency::Amount;
use primitives::{StellarPublicKeyRaw, VaultCurrencyPair, VaultId};
use crate::{CurrencySource, DefaultVault, DefaultVaultId, Error, Event, ext, pool_staking_manager, Vault, VaultStatus};
use crate::types::{CurrencyId, DefaultSystemVault, DefaultVaultCurrencyPair, RichVault, RichSystemVault, UnsignedFixedPoint, BalanceOf};
use crate::pallet::{Call, Config, LiquidationCollateralThreshold, LiquidationVault, MinimumCollateralVault, Pallet, PremiumRedeemThreshold, PunishmentDelay, SecureCollateralThreshold, SystemCollateralCeiling, TotalUserVaultCollateral, Vaults, VaultStellarPublicKey};

fn _offchain_worker<T:Config>() {
    for vault in undercollateralized_vaults() {
        log::info!("Reporting vault {:?}", vault);
        let call = Call::report_undercollateralized_vault { vault_id: vault };
        let _ = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into());
    }
}

/// Public functions

pub fn liquidation_vault_account_id<T:Config>() -> T::AccountId {
    <T as Config>::PalletId::get().into_account_truncating()
}

pub fn _register_vault<T:Config>(
    vault_id: DefaultVaultId<T>,
    collateral: BalanceOf<T>,
) -> DispatchResult {
    ensure!(
			SecureCollateralThreshold::<T>::contains_key(&vault_id.currencies),
			Error::<T>::SecureCollateralThresholdNotSet
		);
    ensure!(
			PremiumRedeemThreshold::<T>::contains_key(&vault_id.currencies),
			Error::<T>::PremiumRedeemThresholdNotSet
		);
    ensure!(
			LiquidationCollateralThreshold::<T>::contains_key(&vault_id.currencies),
			Error::<T>::LiquidationCollateralThresholdNotSet
		);
    ensure!(
			MinimumCollateralVault::<T>::contains_key(vault_id.collateral_currency()),
			Error::<T>::MinimumCollateralNotSet
		);
    ensure!(
			SystemCollateralCeiling::<T>::contains_key(&vault_id.currencies),
			Error::<T>::CeilingNotSet
		);

    // make sure a public key is registered
    let _ = get_stellar_public_key(&vault_id.account_id)?;

    let collateral_currency = vault_id.currencies.collateral;
    let amount = Amount::new(collateral, collateral_currency);

    ensure!(
			amount.ge(&get_minimum_collateral_vault(collateral_currency))?,
			Error::<T>::InsufficientVaultCollateralAmount
		);
    ensure!(!vault_exists(&vault_id), Error::<T>::VaultAlreadyRegistered);

    let vault = Vault::new(vault_id.clone());
    insert_vault(&vault_id, vault);

    try_deposit_collateral(&vault_id, &amount)?;

    Pallet::<T>::deposit_event(Event::<T>::RegisterVault { vault_id, collateral });

    Ok(())
}

pub fn try_set_vault_custom_secure_threshold<T:Config>(
    vault_id: &DefaultVaultId<T>,
    new_threshold: Option<UnsignedFixedPoint<T>>,
) -> DispatchResult {
    if let Some(some_new_threshold) = new_threshold {
        let global_threshold = SecureCollateralThreshold::<T>::get(&vault_id.currencies)
            .ok_or(Error::<T>::GlobalThresholdNotSet)?;
        ensure!(
				some_new_threshold.gt(&global_threshold),
				Error::<T>::ThresholdNotAboveGlobalThreshold
			);
    }
    let mut vault = get_rich_vault_from_id(vault_id)?;
    vault.set_custom_secure_threshold(new_threshold)
}

pub fn get_vault_secure_threshold<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<UnsignedFixedPoint<T>, DispatchError> {
    let vault = get_rich_vault_from_id(vault_id)?;
    vault.get_secure_threshold()
}

pub fn get_stellar_public_key<T:Config>(
    account_id: &T::AccountId,
) -> Result<StellarPublicKeyRaw, DispatchError> {
    VaultStellarPublicKey::<T>::get(account_id)
        .ok_or_else(|| Error::<T>::NoStellarPublicKey.into())
}

pub fn get_vault_from_id<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<DefaultVault<T>, DispatchError> {
    Vaults::<T>::get(vault_id).ok_or_else(|| Error::<T>::VaultNotFound.into())
}

pub fn get_backing_collateral<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<Amount<T>, DispatchError> {
    let stake = ext::staking::total_current_stake::<T>(vault_id)?;
    Ok(Amount::new(stake, vault_id.currencies.collateral))
}

pub fn get_liquidated_collateral<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<Amount<T>, DispatchError> {
    let vault = get_vault_from_id(vault_id)?;
    Ok(Amount::new(vault.liquidated_collateral, vault_id.currencies.collateral))
}

pub fn get_free_redeemable_tokens<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<Amount<T>, DispatchError> {
    get_rich_vault_from_id(vault_id)?.freely_redeemable_tokens()
}

/// Like get_vault_from_id, but additionally checks that the vault is active
pub fn get_active_vault_from_id<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<DefaultVault<T>, DispatchError> {
    let vault = get_vault_from_id(vault_id)?;
    match vault.status {
        VaultStatus::Active(_) => Ok(vault),
        VaultStatus::Liquidated => Err(Error::<T>::VaultLiquidated.into()),
    }
}

/// Deposit an `amount` of collateral to be used for collateral tokens
///
/// # Arguments
/// * `vault_id` - the id of the vault
/// * `amount` - the amount of collateral
pub fn try_deposit_collateral<T:Config>(
    vault_id: &DefaultVaultId<T>,
    amount: &Amount<T>,
) -> DispatchResult {
    // ensure the vault is active
    let _vault = get_active_rich_vault_from_id(vault_id)?;

    // will fail if collateral ceiling exceeded
    try_increase_total_backing_collateral(&vault_id.currencies, amount)?;
    // will fail if free_balance is insufficient
    amount.lock_on(&vault_id.account_id)?;

    // Deposit `amount` of stake in the pool
    pool_staking_manager::PoolManager::deposit_collateral(
        &vault_id,
        &vault_id.account_id,
        &amount.clone(),
    )?;

    Ok(())
}

/// Withdraw an `amount` of collateral without checking collateralization
///
/// # Arguments
/// * `vault_id` - the id of the vault
/// * `amount` - the amount of collateral
pub fn force_withdraw_collateral<T:Config>(
    vault_id: &DefaultVaultId<T>,
    amount: &Amount<T>,
) -> DispatchResult {
    // will fail if reserved_balance is insufficient
    amount.unlock_on(&vault_id.account_id)?;
    decrease_total_backing_collateral(&vault_id.currencies, amount)?;

    // Withdraw `amount` of stake from the pool
    pool_staking_manager::PoolManager::withdraw_collateral(
        &vault_id,
        &vault_id.account_id,
        &amount,
        None,
    )?;

    Ok(())
}

/// Withdraw an `amount` of collateral, ensuring that the vault is sufficiently
/// over-collateralized
///
/// # Arguments
/// * `vault_id` - the id of the vault
/// * `amount` - the amount of collateral
pub fn try_withdraw_collateral<T:Config>(
    vault_id: &DefaultVaultId<T>,
    amount: &Amount<T>,
) -> DispatchResult {
    ensure!(
			is_allowed_to_withdraw_collateral(vault_id, amount)?,
			Error::<T>::InsufficientCollateral
		);
    ensure!(
			is_max_nomination_ratio_preserved(vault_id, amount)?,
			Error::<T>::MaxNominationRatioViolation
		);
    force_withdraw_collateral(vault_id, amount)
}

pub fn is_max_nomination_ratio_preserved<T:Config>(
    vault_id: &DefaultVaultId<T>,
    amount: &Amount<T>,
) -> Result<bool, DispatchError> {
    let vault_collateral = compute_collateral(vault_id)?;
    let backing_collateral = get_backing_collateral(vault_id)?;
    let current_nomination = backing_collateral.checked_sub(&vault_collateral)?;
    let new_vault_collateral = vault_collateral.checked_sub(amount)?;
    let max_nomination_after_withdrawal =
        get_max_nominatable_collateral(&new_vault_collateral, &vault_id.currencies)?;
    current_nomination.le(&max_nomination_after_withdrawal)
}

/// Checks if the vault would be above the secure threshold after withdrawing collateral
pub fn is_allowed_to_withdraw_collateral<T:Config>(
    vault_id: &DefaultVaultId<T>,
    amount: &Amount<T>,
) -> Result<bool, DispatchError> {
    let vault = get_rich_vault_from_id(vault_id)?;

    let new_collateral = match get_backing_collateral(vault_id)?.checked_sub(amount) {
        Ok(x) => x,
        Err(x) if x == ArithmeticError::Underflow.into() => return Ok(false),
        Err(x) => return Err(x),
    };

    let is_below_threshold = Pallet::<T>::is_collateral_below_vault_secure_threshold(
        &new_collateral,
        &vault.backed_tokens()?,
        &vault,
    )?;
    Ok(!is_below_threshold)
}

pub fn transfer_funds_saturated<T:Config>(
    from: CurrencySource<T>,
    to: CurrencySource<T>,
    amount: &Amount<T>,
) -> Result<Amount<T>, DispatchError> {
    let available_amount = from.current_balance(amount.currency())?;
    let amount = if available_amount.lt(amount)? { available_amount } else { amount.clone() };
    transfer_funds(from, to, &amount)?;
    Ok(amount)
}
fn slash_backing_collateral<T:Config>(
    vault_id: &DefaultVaultId<T>,
    amount: &Amount<T>,
) -> DispatchResult {
    amount.unlock_on(&vault_id.account_id)?;
    decrease_total_backing_collateral(&vault_id.currencies, amount)?;

    pool_staking_manager::PoolManager::slash_collateral(&vault_id, &amount)?;

    Ok(())
}

pub fn transfer_funds<T:Config>(
    from: CurrencySource<T>,
    to: CurrencySource<T>,
    amount: &Amount<T>,
) -> DispatchResult {
    match from {
        CurrencySource::Collateral(ref vault_id) => {
            ensure!(
					vault_id.currencies.collateral == amount.currency(),
					Error::<T>::InvalidCurrency
				);
            slash_backing_collateral(vault_id, amount)?;
        },
        CurrencySource::AvailableReplaceCollateral(ref vault_id) => {
            let mut vault = get_active_rich_vault_from_id(vault_id)?;
            vault.decrease_available_replace_collateral(amount)?;
            amount.unlock_on(&from.account_id())?;
        },
        CurrencySource::ActiveReplaceCollateral(ref vault_id) => {
            let mut vault = get_rich_vault_from_id(vault_id)?;
            vault.decrease_active_replace_collateral(amount)?;
            amount.unlock_on(&from.account_id())?;
        },
        CurrencySource::UserGriefing(_) => {
            amount.unlock_on(&from.account_id())?;
        },
        CurrencySource::LiquidatedCollateral(VaultId { ref currencies, .. }) => {
            decrease_total_backing_collateral(currencies, amount)?;
            amount.unlock_on(&from.account_id())?;
        },
        CurrencySource::LiquidationVault(ref currencies) => {
            let mut liquidation_vault = get_rich_liquidation_vault(currencies);
            liquidation_vault.decrease_collateral(amount)?;
            decrease_total_backing_collateral(currencies, amount)?;
            amount.unlock_on(&from.account_id())?;
        },
        CurrencySource::FreeBalance(_) => {
            // do nothing
        },
    };

    // move from sender's free balance to receiver's free balance
    amount.transfer(&from.account_id(), &to.account_id())?;

    // move receiver funds from free balance to specified currency source
    match to {
        CurrencySource::Collateral(ref vault_id) => {
            // todo: do we need to do this for griefing as well?
            ensure!(
					vault_id.currencies.collateral == amount.currency(),
					Error::<T>::InvalidCurrency
				);
            try_deposit_collateral(vault_id, amount)?;
        },
        CurrencySource::AvailableReplaceCollateral(ref vault_id) => {
            let mut vault = get_active_rich_vault_from_id(vault_id)?;
            vault.increase_available_replace_collateral(amount)?;
            amount.lock_on(&to.account_id())?;
        },
        CurrencySource::ActiveReplaceCollateral(ref vault_id) => {
            let mut vault = get_rich_vault_from_id(vault_id)?;
            vault.increase_active_replace_collateral(amount)?;
            amount.lock_on(&to.account_id())?;
        },
        CurrencySource::UserGriefing(_) => {
            amount.lock_on(&to.account_id())?;
        },
        CurrencySource::LiquidatedCollateral(VaultId { ref currencies, .. }) => {
            try_increase_total_backing_collateral(currencies, amount)?;
            amount.lock_on(&to.account_id())?;
        },
        CurrencySource::LiquidationVault(ref currencies) => {
            try_increase_total_backing_collateral(currencies, amount)?;
            let mut liquidation_vault = get_rich_liquidation_vault(currencies);
            liquidation_vault.increase_collateral(amount)?;
            amount.lock_on(&to.account_id())?;
        },
        CurrencySource::FreeBalance(_) => {
            // do nothing
        },
    };

    Ok(())
}

/// Checks if the vault has sufficient collateral to increase the to-be-issued tokens, and
/// if so, increases it
///
/// # Arguments
/// * `vault_id` - the id of the vault from which to increase to-be-issued tokens
/// * `tokens` - the amount of tokens to be reserved
pub fn try_increase_to_be_issued_tokens<T:Config>(
    vault_id: &DefaultVaultId<T>,
    tokens: &Amount<T>,
) -> Result<(), DispatchError> {
    let mut vault = get_active_rich_vault_from_id(vault_id)?;

    let issuable_tokens = vault.issuable_tokens()?;
    ensure!(issuable_tokens.ge(tokens)?, Error::<T>::ExceedingVaultLimit);
    vault.request_issue_tokens(tokens)?;

    Pallet::<T>::deposit_event(Event::<T>::IncreaseToBeIssuedTokens {
        vault_id: vault.id(),
        increase: tokens.amount(),
    });
    Ok(())
}

/// returns the amount of tokens that a vault can request to be replaced on top of the
/// current to-be-replaced tokens
pub fn requestable_to_be_replaced_tokens<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<Amount<T>, DispatchError> {
    let vault = get_active_rich_vault_from_id(vault_id)?;

    vault
        .issued_tokens()
        .checked_sub(&vault.to_be_replaced_tokens())?
        .checked_sub(&vault.to_be_redeemed_tokens())
}

/// returns the new total to-be-replaced and replace-collateral
pub fn try_increase_to_be_replaced_tokens<T:Config>(
    vault_id: &DefaultVaultId<T>,
    tokens: &Amount<T>,
) -> Result<Amount<T>, DispatchError> {
    let mut vault = get_active_rich_vault_from_id(vault_id)?;

    let new_to_be_replaced = vault.to_be_replaced_tokens().checked_add(tokens)?;
    let total_decreasing_tokens =
        new_to_be_replaced.checked_add(&vault.to_be_redeemed_tokens())?;

    ensure!(
			total_decreasing_tokens.le(&vault.issued_tokens())?,
			Error::<T>::InsufficientTokensCommitted
		);

    vault.set_to_be_replaced_amount(&new_to_be_replaced)?;

    Pallet::<T>::deposit_event(Event::<T>::IncreaseToBeReplacedTokens {
        vault_id: vault.id(),
        increase: tokens.amount(),
    });

    Ok(new_to_be_replaced)
}

pub fn decrease_to_be_replaced_tokens<T:Config>(
    vault_id: &DefaultVaultId<T>,
    tokens: &Amount<T>,
) -> Result<(Amount<T>, Amount<T>), DispatchError> {
    let mut vault = get_rich_vault_from_id(vault_id)?;

    let initial_to_be_replaced =
        Amount::new(vault.data.to_be_replaced_tokens, vault_id.wrapped_currency());
    let initial_griefing_collateral =
        Amount::new(vault.data.replace_collateral, T::GetGriefingCollateralCurrencyId::get());

    let used_tokens = tokens.min(&initial_to_be_replaced)?;

    let used_collateral = calculate_collateral(
        &initial_griefing_collateral,
        &used_tokens,
        &initial_to_be_replaced,
    )?;

    // make sure we don't use too much if a rounding error occurs
    let used_collateral = used_collateral.min(&initial_griefing_collateral)?;

    let new_to_be_replaced = initial_to_be_replaced.checked_sub(&used_tokens)?;

    vault.set_to_be_replaced_amount(&new_to_be_replaced)?;

    Pallet::<T>::deposit_event(Event::<T>::DecreaseToBeReplacedTokens {
        vault_id: vault.id(),
        decrease: tokens.amount(),
    });

    Ok((used_tokens, used_collateral))
}

/// Decreases the amount of tokens to be issued in the next issue request from the
/// vault, or from the liquidation vault if the vault is liquidated
///
/// # Arguments
/// * `vault_id` - the id of the vault from which to decrease to-be-issued tokens
/// * `tokens` - the amount of tokens to be unreserved
pub fn decrease_to_be_issued_tokens<T:Config>(
    vault_id: &DefaultVaultId<T>,
    tokens: &Amount<T>,
) -> DispatchResult {
    let mut vault = get_rich_vault_from_id(vault_id)?;
    vault.cancel_issue_tokens(tokens)?;

    Pallet::<T>::deposit_event(Event::<T>::DecreaseToBeIssuedTokens {
        vault_id: vault_id.clone(),
        decrease: tokens.amount(),
    });
    Ok(())
}

/// Issues an amount of `tokens` tokens for the given `vault_id`
/// At this point, the to-be-issued tokens assigned to a vault are decreased
/// and the issued tokens balance is increased by the amount of issued tokens.
///
/// # Arguments
/// * `vault_id` - the id of the vault from which to issue tokens
/// * `tokens` - the amount of tokens to issue
///
/// # Errors
/// * `VaultNotFound` - if no vault exists for the given `vault_id`
/// * `InsufficientTokensCommitted` - if the amount of tokens reserved is too low
pub fn issue_tokens<T:Config>(vault_id: &DefaultVaultId<T>, tokens: &Amount<T>) -> DispatchResult {
    let mut vault = get_rich_vault_from_id(vault_id)?;
    vault.execute_issue_tokens(tokens)?;
    Pallet::<T>::deposit_event(Event::<T>::IssueTokens {
        vault_id: vault.id(),
        increase: tokens.amount(),
    });
    Ok(())
}

/// Adds an amount tokens to the to-be-redeemed tokens balance of a vault.
/// This function serves as a prevention against race conditions in the
/// redeem and replace procedures. If, for example, a vault would receive
/// two redeem requests at the same time that have a higher amount of tokens
///  to be issued than his issuedTokens balance, one of the two redeem
/// requests should be rejected.
///
/// # Arguments
/// * `vault_id` - the id of the vault from which to increase to-be-redeemed tokens
/// * `tokens` - the amount of tokens to be redeemed
///
/// # Errors
/// * `VaultNotFound` - if no vault exists for the given `vault_id`
/// * `InsufficientTokensCommitted` - if the amount of redeemable tokens is too low
pub fn try_increase_to_be_redeemed_tokens<T:Config>(
    vault_id: &DefaultVaultId<T>,
    tokens: &Amount<T>,
) -> DispatchResult {
    let mut vault = get_active_rich_vault_from_id(vault_id)?;
    let redeemable = vault.issued_tokens().checked_sub(&vault.to_be_redeemed_tokens())?;
    ensure!(redeemable.ge(tokens)?, Error::<T>::InsufficientTokensCommitted);

    vault.request_redeem_tokens(tokens)?;

    Pallet::<T>::deposit_event(Event::<T>::IncreaseToBeRedeemedTokens {
        vault_id: vault.id(),
        increase: tokens.amount(),
    });
    Ok(())
}

/// Subtracts an amount tokens from the to-be-redeemed tokens balance of a vault.
///
/// # Arguments
/// * `vault_id` - the id of the vault from which to decrease to-be-redeemed tokens
/// * `tokens` - the amount of tokens to be redeemed
///
/// # Errors
/// * `VaultNotFound` - if no vault exists for the given `vault_id`
/// * `InsufficientTokensCommitted` - if the amount of to-be-redeemed tokens is too low
pub fn decrease_to_be_redeemed_tokens<T:Config>(
    vault_id: &DefaultVaultId<T>,
    tokens: &Amount<T>,
) -> DispatchResult {
    let mut vault = get_rich_vault_from_id(vault_id)?;
    vault.cancel_redeem_tokens(tokens)?;

    Pallet::<T>::deposit_event(Event::<T>::DecreaseToBeRedeemedTokens {
        vault_id: vault.id(),
        decrease: tokens.amount(),
    });
    Ok(())
}

/// Decreases the amount of tokens f a redeem request is not fulfilled
/// Removes the amount of tokens assigned to the to-be-redeemed tokens.
/// At this point, we consider the tokens lost and the issued tokens are
/// removed from the vault
///
/// # Arguments
/// * `vault_id` - the id of the vault from which to decrease tokens
/// * `tokens` - the amount of tokens to be decreased
/// * `user_id` - the id of the user making the redeem request
pub fn decrease_tokens<T:Config>(
    vault_id: &DefaultVaultId<T>,
    user_id: &T::AccountId,
    tokens: &Amount<T>,
) -> DispatchResult {
    // decrease to-be-redeemed and issued
    let mut vault = get_rich_vault_from_id(vault_id)?;
    vault.execute_redeem_tokens(tokens)?;

    Pallet::<T>::deposit_event(Event::<T>::DecreaseTokens {
        vault_id: vault.id(),
        user_id: user_id.clone(),
        decrease: tokens.amount(),
    });
    Ok(())
}

/// Decreases the amount of collateral held after liquidation for any remaining to_be_redeemed
/// tokens.
///
/// # Arguments
/// * `vault_id` - the id of the vault
/// * `amount` - the amount of collateral to decrement
pub fn decrease_liquidated_collateral<T:Config>(
    vault_id: &DefaultVaultId<T>,
    amount: &Amount<T>,
) -> DispatchResult {
    let mut vault = get_rich_vault_from_id(vault_id)?;
    vault.decrease_liquidated_collateral(amount)?;
    Ok(())
}

/// Reduces the to-be-redeemed tokens when a redeem request completes
///
/// # Arguments
/// * `vault_id` - the id of the vault from which to redeem tokens
/// * `tokens` - the amount of tokens to be decreased
/// * `premium` - amount of collateral to be rewarded to the redeemer if the vault is not
///   liquidated yet
/// * `redeemer_id` - the id of the redeemer
pub fn redeem_tokens<T:Config>(
    vault_id: &DefaultVaultId<T>,
    tokens: &Amount<T>,
    premium: &Amount<T>,
    redeemer_id: &T::AccountId,
) -> DispatchResult {
    let mut vault = get_rich_vault_from_id(vault_id)?;

    // need to read before we decrease it
    let to_be_redeemed_tokens = vault.to_be_redeemed_tokens();

    vault.execute_redeem_tokens(tokens)?;

    if !vault.data.is_liquidated() {
        if premium.is_zero() {
            Pallet::<T>::deposit_event(Event::<T>::RedeemTokens {
                vault_id: vault.id(),
                redeemed_amount: tokens.amount(),
            });
        } else {
            transfer_funds(
                CurrencySource::Collateral(vault_id.clone()),
                CurrencySource::FreeBalance(redeemer_id.clone()),
                premium,
            )?;

            Pallet::<T>::deposit_event(Event::<T>::RedeemTokensPremium {
                vault_id: vault_id.clone(),
                redeemed_amount: tokens.amount(),
                collateral: premium.amount(),
                user_id: redeemer_id.clone(),
            });
        }
    } else {
        // NOTE: previously we calculated the amount to release based on the Vault's
        // `backing_collateral` but this may now be wrong in the pull-based approach if the
        // Vault is left with excess collateral
        let to_be_released = calculate_collateral(
            &vault.liquidated_collateral(),
            tokens,
            &to_be_redeemed_tokens,
        )?;
        decrease_total_backing_collateral(&vault_id.currencies, &to_be_released)?;
        vault.decrease_liquidated_collateral(&to_be_released)?;

        // release the collateral back to the free balance of the vault
        to_be_released.unlock_on(&vault_id.account_id)?;

        Pallet::<T>::deposit_event(Event::<T>::RedeemTokensLiquidatedVault {
            vault_id: vault_id.clone(),
            tokens: tokens.amount(),
            collateral: to_be_released.amount(),
        });
    }

    Ok(())
}

/// Handles redeem requests which are executed against the LiquidationVault.
/// Reduces the issued token of the LiquidationVault and slashes the
/// corresponding amount of collateral.
///
/// # Arguments
/// * `currency_id` - the currency being redeemed
/// * `redeemer_id` - the account of the user redeeming issued tokens
/// * `amount_wrapped` - the amount of assets to be redeemed in collateral with the
///   LiquidationVault, denominated in the wrapped asset
///
/// # Errors
/// * `InsufficientTokensCommitted` - if the amount of tokens issued by the liquidation vault is
///   too low
/// * `InsufficientFunds` - if the liquidation vault does not have enough collateral to transfer
pub fn redeem_tokens_liquidation<T:Config>(
    currency_id: CurrencyId<T>,
    redeemer_id: &T::AccountId,
    amount_wrapped: &Amount<T>,
) -> DispatchResult {
    let currency_pair =
        VaultCurrencyPair { collateral: currency_id, wrapped: amount_wrapped.currency() };

    let liquidation_vault = get_rich_liquidation_vault(&currency_pair);

    ensure!(
			liquidation_vault.redeemable_tokens()?.ge(amount_wrapped)?,
			Error::<T>::InsufficientTokensCommitted
		);

    let source_liquidation_vault = CurrencySource::<T>::LiquidationVault(currency_pair.clone());

    // transfer liquidated collateral to redeemer
    let to_transfer = calculate_collateral(
        &source_liquidation_vault.current_balance(currency_id)?,
        amount_wrapped,
        &liquidation_vault.to_be_backed_tokens()?,
    )?;

    transfer_funds(
        source_liquidation_vault,
        CurrencySource::FreeBalance(redeemer_id.clone()),
        &to_transfer,
    )?;

    // need to requery since the liquidation vault gets modified in `transfer_funds`
    let mut liquidation_vault = get_rich_liquidation_vault(&currency_pair);
    liquidation_vault.burn_issued(amount_wrapped)?;

    Pallet::<T>::deposit_event(Event::<T>::RedeemTokensLiquidation {
        redeemer_id: redeemer_id.clone(),
        burned_tokens: amount_wrapped.amount(),
        transferred_collateral: to_transfer.amount(),
    });

    Ok(())
}

/// Replaces the old vault by the new vault by transferring tokens
/// from the old vault to the new one
///
/// # Arguments
/// * `old_vault_id` - the id of the old vault
/// * `new_vault_id` - the id of the new vault
/// * `tokens` - the amount of tokens to be transferred from the old to the new vault
/// * `collateral` - the collateral to be locked by the new vault
///
/// # Errors
/// * `VaultNotFound` - if either the old or new vault does not exist
/// * `InsufficientTokensCommitted` - if the amount of tokens of the old vault is too low
/// * `InsufficientFunds` - if the new vault does not have enough collateral to lock
pub fn replace_tokens<T:Config>(
    old_vault_id: &DefaultVaultId<T>,
    new_vault_id: &DefaultVaultId<T>,
    tokens: &Amount<T>,
    collateral: &Amount<T>,
) -> DispatchResult {
    let mut old_vault = get_rich_vault_from_id(old_vault_id)?;
    let mut new_vault = get_rich_vault_from_id(new_vault_id)?;

    if old_vault.data.is_liquidated() {
        let to_be_released = calculate_collateral(
            &old_vault.liquidated_collateral(),
            tokens,
            &old_vault.to_be_redeemed_tokens(),
        )?;
        old_vault.decrease_liquidated_collateral(&to_be_released)?;

        // deposit old-vault's collateral (this was withdrawn on liquidation)
        pool_staking_manager::PoolManager::deposit_collateral(
            &old_vault_id,
            &old_vault_id.account_id,
            &to_be_released,
        )?;
    }

    old_vault.execute_redeem_tokens(tokens)?;
    new_vault.execute_issue_tokens(tokens)?;

    Pallet::<T>::deposit_event(Event::<T>::ReplaceTokens {
        old_vault_id: old_vault_id.clone(),
        new_vault_id: new_vault_id.clone(),
        amount: tokens.amount(),
        additional_collateral: collateral.amount(),
    });
    Ok(())
}

/// Cancels a replace - which in the normal case decreases the old-vault's
/// to-be-redeemed tokens, and the new-vault's to-be-issued tokens.
/// When one or both of the vaults have been liquidated, this function also
/// updates the liquidation vault.
///
/// # Arguments
/// * `old_vault_id` - the id of the old vault
/// * `new_vault_id` - the id of the new vault
/// * `tokens` - the amount of tokens to be transferred from the old to the new vault
pub fn cancel_replace_tokens<T:Config>(
    old_vault_id: &DefaultVaultId<T>,
    new_vault_id: &DefaultVaultId<T>,
    tokens: &Amount<T>,
) -> DispatchResult {
    let mut old_vault = get_rich_vault_from_id(old_vault_id)?;
    let mut new_vault = get_rich_vault_from_id(new_vault_id)?;

    if old_vault.data.is_liquidated() {
        let to_be_transferred = calculate_collateral(
            &old_vault.liquidated_collateral(),
            tokens,
            &old_vault.to_be_redeemed_tokens(),
        )?;
        old_vault.decrease_liquidated_collateral(&to_be_transferred)?;

        // transfer old-vault's collateral to liquidation_vault
        transfer_funds(
            CurrencySource::LiquidatedCollateral(old_vault_id.clone()),
            CurrencySource::LiquidationVault(old_vault_id.currencies.clone()),
            &to_be_transferred,
        )?;
    }

    old_vault.cancel_redeem_tokens(tokens)?;
    new_vault.cancel_issue_tokens(tokens)?;

    Ok(())
}

/// Withdraws an `amount` of tokens that were requested for replacement by `vault_id`
///
/// # Arguments
/// * `vault_id` - the id of the vault
/// * `amount` - the amount of tokens to be withdrawn from replace requests
pub fn withdraw_replace_request<T:Config>(
    vault_id: &DefaultVaultId<T>,
    amount: &Amount<T>,
) -> Result<(Amount<T>, Amount<T>), DispatchError> {
    let (withdrawn_tokens, to_withdraw_collateral) =
        decrease_to_be_replaced_tokens(vault_id, amount)?;

    // release the used collateral
    transfer_funds(
        CurrencySource::AvailableReplaceCollateral(vault_id.clone()),
        CurrencySource::FreeBalance(vault_id.account_id.clone()),
        &to_withdraw_collateral,
    )?;

    Ok((withdrawn_tokens, to_withdraw_collateral))
}

fn undercollateralized_vaults<T:Config>() -> impl Iterator<Item = DefaultVaultId<T>> {
    <Vaults<T>>::iter().filter_map(|(vault_id, vault)| {
        if let Some(liquidation_threshold) = LiquidationCollateralThreshold::<T>::get(&vault.id.currencies)
        {
            if is_vault_below_liquidation_threshold(&vault, liquidation_threshold)
                .unwrap_or(false)
            {
                return Some(vault_id)
            }
        }
        None
    })
}

/// Liquidates a vault, transferring all of its token balances to the
/// `LiquidationVault`, as well as the collateral.
///
/// # Arguments
/// * `vault_id` - the id of the vault to liquidate
/// * `status` - status with which to liquidate the vault
pub fn liquidate_vault<T:Config>(vault_id: &DefaultVaultId<T>) -> Result<Amount<T>, DispatchError> {
    let mut vault = get_active_rich_vault_from_id(vault_id)?;
    let backing_collateral = vault.get_total_collateral()?;
    let vault_orig = vault.data.clone();

    let to_slash = vault.liquidate()?;

    Pallet::<T>::deposit_event(Event::<T>::LiquidateVault {
        vault_id: vault_id.clone(),
        issued_tokens: vault_orig.issued_tokens,
        to_be_issued_tokens: vault_orig.to_be_issued_tokens,
        to_be_redeemed_tokens: vault_orig.to_be_redeemed_tokens,
        to_be_replaced_tokens: vault_orig.to_be_replaced_tokens,
        backing_collateral: backing_collateral.amount(),
        status: VaultStatus::Liquidated,
        replace_collateral: vault_orig.replace_collateral,
    });
    Ok(to_slash)
}

pub fn try_increase_total_backing_collateral<T:Config>(
    currency_pair: &DefaultVaultCurrencyPair<T>,
    amount: &Amount<T>,
) -> DispatchResult {
    let new = get_total_user_vault_collateral(currency_pair)?.checked_add(amount)?;

    let limit = get_collateral_ceiling(currency_pair)?;
    ensure!(new.le(&limit)?, Error::<T>::CurrencyCeilingExceeded);

    TotalUserVaultCollateral::<T>::insert(currency_pair, new.amount());

    Pallet::<T>::deposit_event(Event::<T>::IncreaseLockedCollateral {
        currency_pair: currency_pair.clone(),
        delta: amount.amount(),
        total: new.amount(),
    });
    Ok(())
}

pub fn decrease_total_backing_collateral<T:Config>(
    currency_pair: &DefaultVaultCurrencyPair<T>,
    amount: &Amount<T>,
) -> DispatchResult {
    let new = get_total_user_vault_collateral(currency_pair)?.checked_sub(amount)?;

    TotalUserVaultCollateral::<T>::insert(currency_pair, new.amount());

    Pallet::<T>::deposit_event(Event::<T>::DecreaseLockedCollateral {
        currency_pair: currency_pair.clone(),
        delta: amount.amount(),
        total: new.amount(),
    });
    Ok(())
}

pub fn insert_vault<T:Config>(id: &DefaultVaultId<T>, vault: DefaultVault<T>) {
    Vaults::<T>::insert(id, vault)
}

pub fn ban_vault<T:Config>(vault_id: &DefaultVaultId<T>) -> DispatchResult {
    let height = ext::security::active_block_number::<T>();
    let mut vault = get_active_rich_vault_from_id(vault_id)?;
    let banned_until = height + PunishmentDelay::<T>::get();

    vault.ban_until(banned_until);
    Pallet::<T>::deposit_event(Event::<T>::BanVault { vault_id: vault.id(), banned_until });
    Ok(())
}

pub fn _ensure_not_banned<T:Config>(vault_id: &DefaultVaultId<T>) -> DispatchResult {
    let vault = get_active_rich_vault_from_id(vault_id)?;
    vault.ensure_not_banned()
}

/// Threshold checks
pub fn is_vault_below_secure_threshold<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<bool, DispatchError> {
    let vault = get_rich_vault_from_id(vault_id)?;
    let threshold = vault.get_secure_threshold()?;
    is_vault_below_threshold(vault_id, threshold)
}

pub fn is_vault_liquidated<T:Config>(vault_id: &DefaultVaultId<T>) -> Result<bool, DispatchError> {
    Ok(get_vault_from_id(vault_id)?.is_liquidated())
}

pub fn is_vault_below_premium_threshold<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<bool, DispatchError> {
    let threshold = PremiumRedeemThreshold::<T>::get(&vault_id.currencies)
        .ok_or(Error::<T>::PremiumRedeemThresholdNotSet)?;
    is_vault_below_threshold(vault_id, threshold)
}

/// check if the vault is below the liquidation threshold.
pub fn is_vault_below_liquidation_threshold<T:Config>(
    vault: &DefaultVault<T>,
    liquidation_threshold: UnsignedFixedPoint<T>,
) -> Result<bool, DispatchError> {
    is_collateral_below_threshold(
        &get_backing_collateral(&vault.id)?,
        &Amount::new(vault.issued_tokens, vault.id.wrapped_currency()),
        liquidation_threshold,
    )
}

/// Takes vault custom secure threshold into account (if set)
pub fn is_collateral_below_vault_secure_threshold<T:Config>(
    collateral: &Amount<T>,
    wrapped_amount: &Amount<T>,
    vault: &RichVault<T>,
) -> Result<bool, DispatchError> {
    let threshold = vault.get_secure_threshold()?;
    is_collateral_below_threshold(collateral, wrapped_amount, threshold)
}

pub fn _set_system_collateral_ceiling<T:Config>(
    currency_pair: DefaultVaultCurrencyPair<T>,
    ceiling: BalanceOf<T>,
) {
    SystemCollateralCeiling::<T>::insert(currency_pair, ceiling);
}

pub fn _set_secure_collateral_threshold<T:Config>(
    currency_pair: DefaultVaultCurrencyPair<T>,
    threshold: UnsignedFixedPoint<T>,
) {
    SecureCollateralThreshold::<T>::insert(currency_pair, threshold);
}

pub fn _set_premium_redeem_threshold<T:Config>(
    currency_pair: DefaultVaultCurrencyPair<T>,
    threshold: UnsignedFixedPoint<T>,
) {
    PremiumRedeemThreshold::<T>::insert(currency_pair, threshold);
}

pub fn _set_liquidation_collateral_threshold<T:Config>(
    currency_pair: DefaultVaultCurrencyPair<T>,
    threshold: UnsignedFixedPoint<T>,
) {
    LiquidationCollateralThreshold::<T>::insert(currency_pair, threshold);
}

/// return (collateral * Numerator) / denominator, used when dealing with liquidated vaults
pub fn calculate_collateral<T:Config>(
    collateral: &Amount<T>,
    numerator: &Amount<T>,
    denominator: &Amount<T>,
) -> Result<Amount<T>, DispatchError> {
    if numerator.is_zero() && denominator.is_zero() {
        return Ok(collateral.clone())
    }

    let currency = collateral.currency();

    let collateral: U256 = collateral.amount().into();
    let numerator: U256 = numerator.amount().into();
    let denominator: U256 = denominator.amount().into();

    let amount = collateral
        .checked_mul(numerator)
        .ok_or(ArithmeticError::Overflow)?
        .checked_div(denominator)
        .ok_or(ArithmeticError::Underflow)?
        .try_into()
        .map_err(|_| Error::<T>::TryIntoIntError)?;
    Ok(Amount::new(amount, currency))
}

/// RPC

/// get all vaults that are registered using the given account id. Note that one account id might
/// be used in multiple vault ids.
pub fn get_vaults_by_account_id<T:Config>(
    account_id: T::AccountId,
) -> Result<Vec<DefaultVaultId<T>>, DispatchError> {
    let vaults = Vaults::<T>::iter()
        .filter(|(vault_id, _)| vault_id.account_id == account_id)
        .map(|(vault_id, _)| vault_id)
        .collect();
    Ok(vaults)
}

/// Get all vaults that:
/// - are below the premium redeem threshold, and
/// - have a non-zero amount of redeemable tokens, and thus
/// - are not banned
///
/// Maybe returns a tuple of (VaultId, RedeemableTokens)
/// The redeemable tokens are the currently vault.issued_tokens - the
/// vault.to_be_redeemed_tokens
#[allow(clippy::type_complexity)]
pub fn get_premium_redeem_vaults<T:Config>() -> Result<Vec<(DefaultVaultId<T>, Amount<T>)>, DispatchError>
{
    let mut suitable_vaults = Vaults::<T>::iter()
        .filter_map(|(vault_id, vault)| {
            let rich_vault: RichVault<T> = vault.into();

            let redeemable_tokens = rich_vault.redeemable_tokens().ok()?;

            if !redeemable_tokens.is_zero() &&
                is_vault_below_premium_threshold(

                    &vault_id).unwrap_or(false)
            {
                Some((vault_id, redeemable_tokens))
            } else {
                None
            }
        })
        .collect::<Vec<(_, _)>>();

    if suitable_vaults.is_empty() {
        Err(Error::<T>::NoVaultUnderThePremiumRedeemThreshold.into())
    } else {
        suitable_vaults.sort_by(|a, b| b.1.amount().cmp(&a.1.amount()));
        Ok(suitable_vaults)
    }
}

/// Get all vaults with non-zero issuable tokens, ordered in descending order of this amount
#[allow(clippy::type_complexity)]
pub fn get_vaults_with_issuable_tokens<T:Config>(
) -> Result<Vec<(DefaultVaultId<T>, Amount<T>)>, DispatchError> {
    let mut vaults_with_issuable_tokens = Vaults::<T>::iter()
        .filter_map(|(vault_id, _vault)| {
            // NOTE: we are not checking if the vault accepts new issues here - if not, then
            // get_issuable_tokens_from_vault will return 0, and we will filter them out below

            // iterator returns tuple of (AccountId, Vault<T>),
            match get_issuable_tokens_from_vault(&vault_id).ok() {
                Some(issuable_tokens) =>
                    if !issuable_tokens.is_zero() {
                        Some((vault_id, issuable_tokens))
                    } else {
                        None
                    },
                None => None,
            }
        })
        .collect::<Vec<(_, _)>>();

    vaults_with_issuable_tokens.sort_by(|a, b| b.1.amount().cmp(&a.1.amount()));
    Ok(vaults_with_issuable_tokens)
}

/// Get all vaults with non-zero issued (thus redeemable) tokens, ordered in descending order of
/// this amount
#[allow(clippy::type_complexity)]
pub fn get_vaults_with_redeemable_tokens<T:Config>(
) -> Result<Vec<(DefaultVaultId<T>, Amount<T>)>, DispatchError> {
    // find all vault accounts with sufficient collateral
    let mut vaults_with_redeemable_tokens = Vaults::<T>::iter()
        .filter_map(|(vault_id, vault)| {
            let vault = Into::<RichVault<T>>::into(vault);
            let redeemable_tokens = vault.redeemable_tokens().ok()?;
            if !redeemable_tokens.is_zero() {
                Some((vault_id, redeemable_tokens))
            } else {
                None
            }
        })
        .collect::<Vec<(_, _)>>();

    vaults_with_redeemable_tokens.sort_by(|a, b| b.1.amount().cmp(&a.1.amount()));
    Ok(vaults_with_redeemable_tokens)
}

/// Get the amount of tokens a vault can issue
pub fn get_issuable_tokens_from_vault<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<Amount<T>, DispatchError> {
    let vault = get_active_rich_vault_from_id(vault_id)?;
    // make sure the vault accepts new issue requests.
    // NOTE: get_vaults_with_issuable_tokens depends on this check
    if vault.data.status != VaultStatus::Active(true) {
        Ok(Amount::new(0u32.into(), vault_id.currencies.collateral))
    } else {
        vault.issuable_tokens()
    }
}

pub fn ensure_accepting_new_issues<T:Config>(vault_id: &DefaultVaultId<T>) -> Result<(), DispatchError> {
    let vault = get_active_rich_vault_from_id(vault_id)?;
    ensure!(
			matches!(vault.data.status, VaultStatus::Active(true)),
			Error::<T>::VaultNotAcceptingIssueRequests
		);
    Ok(())
}

/// Get the amount of tokens issued by a vault
pub fn get_to_be_issued_tokens_from_vault<T:Config>(
    vault_id: DefaultVaultId<T>,
) -> Result<Amount<T>, DispatchError> {
    let vault = get_active_rich_vault_from_id(&vault_id)?;
    Ok(vault.to_be_issued_tokens())
}

/// Get the current collateralization of a vault
pub fn get_collateralization_from_vault<T:Config>(
    vault_id: DefaultVaultId<T>,
    only_issued: bool,
) -> Result<UnsignedFixedPoint<T>, DispatchError> {
    let vault = get_active_rich_vault_from_id(&vault_id)?;
    let collateral = vault.get_total_collateral()?;
    get_collateralization_from_vault_and_collateral(vault_id, &collateral, only_issued)
}

pub fn get_collateralization_from_vault_and_collateral<T:Config>(
    vault_id: DefaultVaultId<T>,
    collateral: &Amount<T>,
    only_issued: bool,
) -> Result<UnsignedFixedPoint<T>, DispatchError> {
    let vault = get_active_rich_vault_from_id(&vault_id)?;
    let issued_tokens =
        if only_issued { vault.issued_tokens() } else { vault.backed_tokens()? };

    ensure!(!issued_tokens.is_zero(), Error::<T>::NoTokensIssued);

    // convert the collateral to wrapped
    let collateral_in_wrapped = collateral.convert_to(vault_id.wrapped_currency())?;

    get_collateralization(&collateral_in_wrapped, &issued_tokens)
}

/// Gets the minimum amount of collateral required for the given amount of Stellar assets
/// with the current threshold and exchange rate
///
/// # Arguments
/// * `amount_wrapped` - the amount of wrapped
/// * `currency_id` - the collateral currency
pub fn get_required_collateral_for_wrapped<T:Config>(
    amount_wrapped: &Amount<T>,
    currency_id: CurrencyId<T>,
) -> Result<Amount<T>, DispatchError> {
    let currency_pair =
        VaultCurrencyPair { collateral: currency_id, wrapped: amount_wrapped.currency() };
    let threshold = SecureCollateralThreshold::<T>::get(&currency_pair)
        .ok_or(Error::<T>::SecureCollateralThresholdNotSet)?;
    let collateral = get_required_collateral_for_wrapped_with_threshold(
        amount_wrapped,
        threshold,
        currency_id,
    )?;
    Ok(collateral)
}

/// Get the amount of collateral required for the given vault to be at the
/// current SecureCollateralThreshold with the current exchange rate
pub fn get_required_collateral_for_vault<T:Config>(
    vault_id: DefaultVaultId<T>,
) -> Result<Amount<T>, DispatchError> {
    let vault = get_active_rich_vault_from_id(&vault_id)?;
    let issued_tokens = vault.backed_tokens()?;

    let threshold = vault.get_secure_threshold()?;
    let required_collateral = get_required_collateral_for_wrapped_with_threshold(
        &issued_tokens,
        threshold,
        vault_id.currencies.collateral,
    )?;

    Ok(required_collateral)
}

pub fn vault_exists<T:Config>(vault_id: &DefaultVaultId<T>) -> bool {
    Vaults::<T>::contains_key(vault_id)
}

pub fn compute_collateral<T:Config>(vault_id: &DefaultVaultId<T>) -> Result<Amount<T>, DispatchError> {
    let amount = ext::staking::compute_stake::<T>(vault_id, &vault_id.account_id)?;
    Ok(Amount::new(amount, vault_id.currencies.collateral))
}

pub fn get_max_nomination_ratio<T:Config>(
    currency_pair: &DefaultVaultCurrencyPair<T>,
) -> Result<UnsignedFixedPoint<T>, DispatchError> {
    // MaxNominationRatio = (SecureCollateralThreshold / PremiumRedeemThreshold) - 1)
    // It denotes the maximum amount of collateral that can be nominated to a particular Vault.
    // Its effect is to minimise the impact on collateralization of nominator withdrawals.
    let secure_collateral_threshold = SecureCollateralThreshold::<T>::get(currency_pair)
        .ok_or(Error::<T>::SecureCollateralThresholdNotSet)?;
    let premium_redeem_threshold = PremiumRedeemThreshold::<T>::get(currency_pair)
        .ok_or(Error::<T>::PremiumRedeemThresholdNotSet)?;
    Ok(secure_collateral_threshold
        .checked_div(&premium_redeem_threshold)
        .ok_or(ArithmeticError::Underflow)?
        .checked_sub(&UnsignedFixedPoint::<T>::one())
        .ok_or(ArithmeticError::Underflow)?)
}

pub fn get_max_nominatable_collateral<T:Config>(
    vault_collateral: &Amount<T>,
    currency_pair: &DefaultVaultCurrencyPair<T>,
) -> Result<Amount<T>, DispatchError> {
    vault_collateral.rounded_mul(get_max_nomination_ratio(currency_pair)?)
}

/// Private getters and setters

fn get_collateral_ceiling<T:Config>(
    currency_pair: &DefaultVaultCurrencyPair<T>,
) -> Result<Amount<T>, DispatchError> {
    let ceiling_amount =
        SystemCollateralCeiling::<T>::get(currency_pair).ok_or(Error::<T>::CeilingNotSet)?;
    Ok(Amount::new(ceiling_amount, currency_pair.collateral))
}

fn get_total_user_vault_collateral<T:Config>(
    currency_pair: &DefaultVaultCurrencyPair<T>,
) -> Result<Amount<T>, DispatchError> {
    Ok(Amount::new(TotalUserVaultCollateral::<T>::get(currency_pair), currency_pair.collateral))
}

fn get_rich_vault_from_id<T:Config>(vault_id: &DefaultVaultId<T>) -> Result<RichVault<T>, DispatchError> {
    Ok(get_vault_from_id(vault_id)?.into())
}

/// Like get_rich_vault_from_id, but only returns active vaults
fn get_active_rich_vault_from_id<T:Config>(
    vault_id: &DefaultVaultId<T>,
) -> Result<RichVault<T>, DispatchError> {
    Ok(get_active_vault_from_id(vault_id)?.into())
}

pub fn get_liquidation_vault<T:Config>(
    currency_pair: &DefaultVaultCurrencyPair<T>,
) -> DefaultSystemVault<T> {
    if let Some(liquidation_vault) = LiquidationVault::<T>::get(currency_pair) {
        liquidation_vault
    } else {
        DefaultSystemVault::<T> {
            to_be_issued_tokens: 0u32.into(),
            issued_tokens: 0u32.into(),
            to_be_redeemed_tokens: 0u32.into(),
            collateral: 0u32.into(),
            currency_pair: currency_pair.clone(),
        }
    }
}

fn get_rich_liquidation_vault<T:Config>(
    currency_pair: &DefaultVaultCurrencyPair<T>,
) -> RichSystemVault<T> {
    get_liquidation_vault(currency_pair).into()
}

fn get_minimum_collateral_vault<T:Config>(currency_id: CurrencyId<T>) -> Amount<T> {
    let amount = MinimumCollateralVault::<T>::get(currency_id);
    Amount::new(amount, currency_id)
}

/// calculate the collateralization as a ratio of the issued tokens to the
/// amount of provided collateral at the current exchange rate.
fn get_collateralization<T:Config>(
    collateral_in_wrapped: &Amount<T>,
    issued_tokens: &Amount<T>,
) -> Result<UnsignedFixedPoint<T>, DispatchError> {
    collateral_in_wrapped.ratio(issued_tokens)
}

fn is_vault_below_threshold<T:Config>(
    vault_id: &DefaultVaultId<T>,
    threshold: UnsignedFixedPoint<T>,
) -> Result<bool, DispatchError> {
    let vault = get_rich_vault_from_id(vault_id)?;

    // the current locked backing collateral by the vault
    let collateral = get_backing_collateral(vault_id)?;

    is_collateral_below_threshold(&collateral, &vault.issued_tokens(), threshold)
}

fn is_collateral_below_threshold<T:Config>(
    collateral: &Amount<T>,
    wrapped_amount: &Amount<T>,
    threshold: UnsignedFixedPoint<T>,
) -> Result<bool, DispatchError> {
    let max_tokens = calculate_max_wrapped_from_collateral_for_threshold(
        collateral,
        wrapped_amount.currency(),
        threshold,
    )?;
    // check if the max_tokens are below the issued tokens
    max_tokens.lt(wrapped_amount)
}

/// Gets the minimum amount of collateral required for the given amount of Stellar assets
/// with the current exchange rate and the given threshold. This function is the
/// inverse of calculate_max_wrapped_from_collateral_for_threshold
///
/// # Arguments
/// * `amount` - the amount of wrapped
/// * `threshold` - the required secure collateral threshold
fn get_required_collateral_for_wrapped_with_threshold<T:Config>(
    wrapped: &Amount<T>,
    threshold: UnsignedFixedPoint<T>,
    currency_id: CurrencyId<T>,
) -> Result<Amount<T>, DispatchError> {
    wrapped.checked_fixed_point_mul_rounded_up(&threshold)?.convert_to(currency_id)
}

fn calculate_max_wrapped_from_collateral_for_threshold<T:Config>(
    collateral: &Amount<T>,
    wrapped_currency: CurrencyId<T>,
    threshold: UnsignedFixedPoint<T>,
) -> Result<Amount<T>, DispatchError> {
    collateral.convert_to(wrapped_currency)?.checked_div(&threshold)
}

