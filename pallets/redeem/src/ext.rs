#[cfg(test)]
use mocktopus::macros::mockable;

#[cfg_attr(test, mockable)]
pub(crate) mod stellar_relay {
	use substrate_stellar_sdk::{
		compound_types::UnlimitedVarArray,
		types::{ScpEnvelope, TransactionSet},
		TransactionEnvelope, XdrCodec,
	};

	use stellar_relay::Error;

	pub fn validate_stellar_transaction<T: crate::Config>(
		transaction_envelope: &TransactionEnvelope,
		envelopes: &UnlimitedVarArray<ScpEnvelope>,
		transaction_set: &TransactionSet,
	) -> Result<(), Error<T>> {
		<stellar_relay::Pallet<T>>::validate_stellar_transaction(
			transaction_envelope,
			envelopes,
			transaction_set,
			// TODO change this
			true,
		)
	}

	pub fn construct_from_raw_encoded_xdr<T: crate::Config, V: XdrCodec>(
		raw_encoded_xdr: &[u8],
	) -> Result<V, Error<T>> {
		<stellar_relay::Pallet<T>>::construct_from_raw_encoded_xdr(raw_encoded_xdr)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod vault_registry {
	use frame_support::dispatch::{DispatchError, DispatchResult};

	use currency::Amount;
	use vault_registry::types::{CurrencyId, CurrencySource, DefaultVault};

	use crate::DefaultVaultId;

	pub fn get_liquidated_collateral<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<Amount<T>, DispatchError> {
		<vault_registry::Pallet<T>>::get_liquidated_collateral(vault_id)
	}

	pub fn get_free_redeemable_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<Amount<T>, DispatchError> {
		<vault_registry::Pallet<T>>::get_free_redeemable_tokens(vault_id)
	}

	pub fn transfer_funds<T: crate::Config>(
		from: CurrencySource<T>,
		to: CurrencySource<T>,
		amount: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::transfer_funds(from, to, amount)
	}

	pub fn transfer_funds_saturated<T: crate::Config>(
		from: CurrencySource<T>,
		to: CurrencySource<T>,
		amount: &Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		<vault_registry::Pallet<T>>::transfer_funds_saturated(from, to, amount)
	}

	pub fn get_vault_from_id<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<DefaultVault<T>, DispatchError> {
		<vault_registry::Pallet<T>>::get_vault_from_id(vault_id)
	}

	pub fn try_increase_to_be_redeemed_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::try_increase_to_be_redeemed_tokens(vault_id, amount)
	}

	pub fn redeem_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		tokens: &Amount<T>,
		premium: &Amount<T>,
		redeemer_id: &T::AccountId,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::redeem_tokens(vault_id, tokens, premium, redeemer_id)
	}

	pub fn decrease_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		user_id: &T::AccountId,
		tokens: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::decrease_tokens(vault_id, user_id, tokens)
	}

	pub fn decrease_liquidated_collateral<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::decrease_liquidated_collateral(vault_id, amount)
	}

	pub fn redeem_tokens_liquidation<T: crate::Config>(
		currency_id: CurrencyId<T>,
		redeemer_id: &T::AccountId,
		amount: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::redeem_tokens_liquidation(currency_id, redeemer_id, amount)
	}

	pub fn ban_vault<T: crate::Config>(vault_id: &DefaultVaultId<T>) -> DispatchResult {
		<vault_registry::Pallet<T>>::ban_vault(vault_id)
	}

	pub fn ensure_not_banned<T: crate::Config>(vault_id: &DefaultVaultId<T>) -> DispatchResult {
		<vault_registry::Pallet<T>>::_ensure_not_banned(vault_id)
	}

	pub fn is_vault_below_premium_threshold<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<bool, DispatchError> {
		<vault_registry::Pallet<T>>::is_vault_below_premium_threshold(vault_id)
	}

	pub fn is_vault_below_secure_threshold<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<bool, DispatchError> {
		<vault_registry::Pallet<T>>::is_vault_below_secure_threshold(vault_id)
	}

	pub fn decrease_to_be_redeemed_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		tokens: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::decrease_to_be_redeemed_tokens(vault_id, tokens)
	}

	pub fn calculate_collateral<T: crate::Config>(
		collateral: &Amount<T>,
		numerator: &Amount<T>,
		denominator: &Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		<vault_registry::Pallet<T>>::calculate_collateral(collateral, numerator, denominator)
	}

	pub fn try_increase_to_be_issued_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> Result<(), DispatchError> {
		<vault_registry::Pallet<T>>::try_increase_to_be_issued_tokens(vault_id, amount)
	}

	pub fn issue_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::issue_tokens(vault_id, amount)
	}

	pub fn decrease_to_be_replaced_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		tokens: &Amount<T>,
	) -> Result<(Amount<T>, Amount<T>), DispatchError> {
		<vault_registry::Pallet<T>>::decrease_to_be_replaced_tokens(vault_id, tokens)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod treasury {
	use currency::Amount;
	use vault_registry::types::CurrencyId;

	pub fn get_balance<T: crate::Config>(
		account: &T::AccountId,
		currency_id: CurrencyId<T>,
	) -> Amount<T> {
		currency::get_free_balance::<T>(currency_id, account)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod security {
	use frame_support::dispatch::DispatchResult;
	use sp_core::H256;
	use sp_runtime::DispatchError;

	pub fn parachain_block_expired<T: crate::Config>(
		opentime: T::BlockNumber,
		period: T::BlockNumber,
	) -> Result<bool, DispatchError> {
		<security::Pallet<T>>::parachain_block_expired(opentime, period)
	}

	pub fn get_secure_id<T: crate::Config>(id: &T::AccountId) -> H256 {
		<security::Pallet<T>>::get_secure_id(id)
	}

	pub fn ensure_parachain_status_running<T: crate::Config>() -> DispatchResult {
		<security::Pallet<T>>::ensure_parachain_status_running()
	}

	pub fn active_block_number<T: crate::Config>() -> T::BlockNumber {
		<security::Pallet<T>>::active_block_number()
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod oracle {
	use frame_support::dispatch::DispatchError;

	use oracle::types::UnsignedFixedPoint;

	use crate::OracleKey;

	pub fn get_price<T: crate::Config>(
		key: OracleKey,
	) -> Result<UnsignedFixedPoint<T>, DispatchError> {
		<oracle::Pallet<T>>::get_price(key)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod fee {
	use frame_support::dispatch::{DispatchError, DispatchResult};

	use currency::Amount;

	pub fn fee_pool_account_id<T: crate::Config>() -> T::AccountId {
		<fee::Pallet<T>>::fee_pool_account_id()
	}

	pub fn get_redeem_fee<T: crate::Config>(
		amount: &Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		<fee::Pallet<T>>::get_redeem_fee(amount)
	}

	pub fn distribute_rewards<T: crate::Config>(amount: &Amount<T>) -> DispatchResult {
		<fee::Pallet<T>>::distribute_rewards(amount)
	}

	pub fn get_punishment_fee<T: crate::Config>(
		amount: &Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		<fee::Pallet<T>>::get_punishment_fee(amount)
	}

	pub fn get_premium_redeem_fee<T: crate::Config>(
		amount: &Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		<fee::Pallet<T>>::get_premium_redeem_fee(amount)
	}
}
