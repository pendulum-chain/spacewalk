#[cfg(test)]
use mocktopus::macros::mockable;

#[cfg_attr(test, mockable)]
pub(crate) mod currency {
	use primitives::stellar::TransactionEnvelope;

	use currency::{Amount, Error};
	use primitives::StellarPublicKeyRaw;

	use crate::types::CurrencyId;

	pub fn get_amount_from_transaction_envelope<T: crate::Config>(
		transaction_envelope: &TransactionEnvelope,
		recipient_stellar_address: StellarPublicKeyRaw,
		currency: CurrencyId<T>,
	) -> Result<Amount<T>, Error<T>> {
		<currency::Pallet<T>>::get_amount_from_transaction_envelope(
			transaction_envelope,
			recipient_stellar_address,
			currency,
		)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod stellar_relay {
	use primitives::{
		stellar::{
			compound_types::UnlimitedVarArray, types::ScpEnvelope, TransactionEnvelope,
			TransactionSetType, XdrCodec,
		},
		TextMemo,
	};

	use stellar_relay::Error;

	pub fn validate_stellar_transaction<T: crate::Config>(
		transaction_envelope: &TransactionEnvelope,
		envelopes: &UnlimitedVarArray<ScpEnvelope>,
		transaction_set: &TransactionSetType,
	) -> Result<(), Error<T>> {
		<stellar_relay::Pallet<T>>::validate_stellar_transaction(
			transaction_envelope,
			envelopes,
			transaction_set,
		)
	}

	pub fn ensure_transaction_memo_matches<T: crate::Config>(
		transaction_envelope: &TransactionEnvelope,
		expected_memo: &TextMemo,
	) -> Result<(), Error<T>> {
		<stellar_relay::Pallet<T>>::ensure_transaction_memo_matches(
			transaction_envelope,
			expected_memo,
		)
	}

	pub fn construct_from_raw_encoded_xdr<T: crate::Config, V: XdrCodec>(
		raw_encoded_xdr: &[u8],
	) -> Result<V, Error<T>> {
		<stellar_relay::Pallet<T>>::construct_from_raw_encoded_xdr(raw_encoded_xdr)
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod security {
	use frame_support::sp_runtime::DispatchError;
	use frame_system::pallet_prelude::BlockNumberFor;
	use sp_core::H256;

	pub fn parachain_block_expired<T: crate::Config>(
		opentime: BlockNumberFor<T>,
		period: BlockNumberFor<T>,
	) -> Result<bool, DispatchError> {
		<security::Pallet<T>>::parachain_block_expired(opentime, period)
	}

	pub fn get_secure_id<T: crate::Config>() -> H256 {
		<security::Pallet<T>>::get_secure_id()
	}

	pub fn active_block_number<T: crate::Config>() -> BlockNumberFor<T> {
		<security::Pallet<T>>::active_block_number()
	}
}

#[cfg_attr(test, mockable)]
pub(crate) mod vault_registry {
	use frame_support::{dispatch::DispatchResult, sp_runtime::DispatchError};

	use primitives::StellarPublicKeyRaw;
	use vault_registry::{
		types::{CurrencySource, DefaultVault},
		Amount,
	};

	use crate::DefaultVaultId;

	pub fn transfer_funds<T: crate::Config>(
		from: CurrencySource<T>,
		to: CurrencySource<T>,
		amount: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::transfer_funds(from, to, amount)
	}

	pub fn is_vault_liquidated<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<bool, DispatchError> {
		<vault_registry::Pallet<T>>::is_vault_liquidated(vault_id)
	}

	pub fn get_active_vault_from_id<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<DefaultVault<T>, DispatchError> {
		<vault_registry::Pallet<T>>::get_active_vault_from_id(vault_id)
	}

	pub fn try_increase_to_be_issued_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> Result<(), DispatchError> {
		<vault_registry::Pallet<T>>::try_increase_to_be_issued_tokens(vault_id, amount)
	}

	pub fn ensure_accepting_new_issues<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<(), DispatchError> {
		<vault_registry::Pallet<T>>::ensure_accepting_new_issues(vault_id)
	}

	pub fn get_issuable_tokens_from_vault<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
	) -> Result<Amount<T>, DispatchError> {
		<vault_registry::Pallet<T>>::get_issuable_tokens_from_vault(vault_id)
	}

	pub fn get_stellar_public_key<T: crate::Config>(
		account_id: &T::AccountId,
	) -> Result<StellarPublicKeyRaw, DispatchError> {
		<vault_registry::Pallet<T>>::get_stellar_public_key(account_id)
	}

	pub fn issue_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		amount: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::issue_tokens(vault_id, amount)
	}

	pub fn ensure_not_banned<T: crate::Config>(vault_id: &DefaultVaultId<T>) -> DispatchResult {
		<vault_registry::Pallet<T>>::_ensure_not_banned(vault_id)
	}

	pub fn decrease_to_be_issued_tokens<T: crate::Config>(
		vault_id: &DefaultVaultId<T>,
		tokens: &Amount<T>,
	) -> DispatchResult {
		<vault_registry::Pallet<T>>::decrease_to_be_issued_tokens(vault_id, tokens)
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
pub(crate) mod fee {
	use frame_support::{dispatch::DispatchResult, sp_runtime::DispatchError};

	use currency::Amount;

	pub fn fee_pool_account_id<T: crate::Config>() -> T::AccountId {
		<fee::Pallet<T>>::fee_pool_account_id()
	}

	pub fn get_issue_fee<T: crate::Config>(amount: &Amount<T>) -> Result<Amount<T>, DispatchError> {
		<fee::Pallet<T>>::get_issue_fee(amount)
	}

	pub fn get_issue_griefing_collateral<T: crate::Config>(
		amount: &Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		<fee::Pallet<T>>::get_issue_griefing_collateral(amount)
	}

	pub fn distribute_rewards<T: crate::Config>(amount: &Amount<T>) -> DispatchResult {
		<fee::Pallet<T>>::distribute_rewards(amount)
	}
}
