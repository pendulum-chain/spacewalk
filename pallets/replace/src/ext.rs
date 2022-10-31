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
			false,
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
	use crate::DefaultVaultId;
	use currency::Amount;
	use frame_support::dispatch::{DispatchError, DispatchResult};
	use vault_registry::types::CurrencySource;

	pub fn transfer_funds<T: crate::Config>(
		from: CurrencySource<T>,
		to: CurrencySource<T>,
		amount: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::transfer_funds(from, to, amount)
	}

	pub fn replace_tokens<T: crate::Config>(
		old_vault_id: &DefaultVaultId<T>,
		new_vault_id: &DefaultVaultId<T>,
		tokens: &Amount<T>,
		collateral: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::replace_tokens(old_vault_id, new_vault_id, tokens, collateral)
	}

	pub fn cancel_replace_tokens<T: crate::Config>(
		old_vault_id: &DefaultVaultId<T>,
		new_vault_id: &DefaultVaultId<T>,
		tokens: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::cancel_replace_tokens(old_vault_id, new_vault_id, tokens)
	}

	pub fn is_vault_liquidated<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<bool, DispatchError> {
		<vault_registry::Pallet<T>>::is_vault_liquidated(vault_id)
	}

	pub fn try_increase_to_be_redeemed_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		tokens: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::try_increase_to_be_redeemed_tokens(vault_id, tokens)
	}

	pub fn ensure_not_banned<T: crate::Config>(vault_id: &DefaultVaultId<T>) -> DispatchResult {
		<vault_registry::Pallet<T>>::_ensure_not_banned(vault_id)
	}

	pub fn try_increase_to_be_issued_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> Result<(), DispatchError> {
		<vault_registry::Pallet<T>>::try_increase_to_be_issued_tokens(vault_id, amount)
	}

	pub fn requestable_to_be_replaced_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<Amount<T>, DispatchError> {
		<vault_registry::Pallet<T>>::requestable_to_be_replaced_tokens(vault_id)
	}

	pub fn try_increase_to_be_replaced_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		<vault_registry::Pallet<T>>::try_increase_to_be_replaced_tokens(vault_id, amount)
	}

	pub fn decrease_to_be_replaced_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		tokens: &Amount<T>,
	) -> Result<(Amount<T>, Amount<T>), DispatchError> {
		<vault_registry::Pallet<T>>::decrease_to_be_replaced_tokens(vault_id, tokens)
	}

	pub fn withdraw_replace_request<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> Result<(Amount<T>, Amount<T>), DispatchError> {
		<vault_registry::Pallet<T>>::withdraw_replace_request(vault_id, amount)
	}

	pub fn try_deposit_collateral<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> Result<(), DispatchError> {
		<vault_registry::Pallet<T>>::try_deposit_collateral(vault_id, amount)
	}

	pub fn force_withdraw_collateral<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> Result<(), DispatchError> {
		<vault_registry::Pallet<T>>::force_withdraw_collateral(vault_id, amount)
	}

	pub fn is_allowed_to_withdraw_collateral<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> Result<bool, DispatchError> {
		<vault_registry::Pallet<T>>::is_allowed_to_withdraw_collateral(vault_id, amount)
	}

	pub fn calculate_collateral<T: crate::Config>(
		collateral: &Amount<T>,
		numerator: &Amount<T>,
		denominator: &Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		<vault_registry::Pallet<T>>::calculate_collateral(collateral, numerator, denominator)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod security {
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

	pub fn active_block_number<T: crate::Config>() -> T::BlockNumber {
		<security::Pallet<T>>::active_block_number()
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod fee {
	use currency::Amount;
	use frame_support::dispatch::DispatchError;

	pub fn get_replace_griefing_collateral<T: crate::Config>(
		amount: &Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		<fee::Pallet<T>>::get_replace_griefing_collateral(amount)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod nomination {
	use crate::DefaultVaultId;
	use sp_runtime::DispatchError;

	pub fn is_nominatable<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<bool, DispatchError> {
		<nomination::Pallet<T>>::is_opted_in(vault_id)
	}
}
