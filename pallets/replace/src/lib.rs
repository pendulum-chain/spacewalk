//! # Replace Pallet
//! Based on the [specification](https://spec.interlay.io/spec/replace.html).

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
extern crate mocktopus;

use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	ensure, log,
	traits::Get,
	transactional,
};
#[cfg(test)]
use mocktopus::macros::mockable;
use sp_core::H256;
use sp_std::vec::Vec;
use substrate_stellar_sdk::{
	compound_types::UnlimitedVarArray,
	types::{ScpEnvelope, TransactionSet},
	TransactionEnvelope,
};

use currency::Amount;
pub use default_weights::{SubstrateWeight, WeightInfo};
pub use pallet::*;
use primitives::StellarPublicKeyRaw;
use types::DefaultVaultId;
use vault_registry::{types::CurrencyId, CurrencySource};

use crate::types::{BalanceOf, ReplaceRequestExt};
pub use crate::types::{DefaultReplaceRequest, ReplaceRequest, ReplaceRequestStatus};

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod default_weights;
mod ext;

pub mod types;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use primitives::{StellarPublicKeyRaw, VaultId};
	use vault_registry::types::DefaultVaultCurrencyPair;

	use super::*;

	/// ## Configuration
	/// The pallet's configuration trait.
	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ vault_registry::Config
		+ stellar_relay::Config
		+ oracle::Config
		+ fee::Config
		+ nomination::Config
	{
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		RequestReplace {
			old_vault_id: DefaultVaultId<T>,
			amount: BalanceOf<T>,
			asset: CurrencyId<T>,
			griefing_collateral: BalanceOf<T>,
		},
		WithdrawReplace {
			old_vault_id: DefaultVaultId<T>,
			withdrawn_tokens: BalanceOf<T>,
			asset: CurrencyId<T>,
			withdrawn_griefing_collateral: BalanceOf<T>,
		},
		AcceptReplace {
			replace_id: H256,
			old_vault_id: DefaultVaultId<T>,
			new_vault_id: DefaultVaultId<T>,
			amount: BalanceOf<T>,
			asset: CurrencyId<T>,
			collateral: BalanceOf<T>,
			stellar_address: StellarPublicKeyRaw,
		},
		ExecuteReplace {
			replace_id: H256,
			old_vault_id: DefaultVaultId<T>,
			new_vault_id: DefaultVaultId<T>,
		},
		CancelReplace {
			replace_id: H256,
			new_vault_id: DefaultVaultId<T>,
			old_vault_id: DefaultVaultId<T>,
			griefing_collateral: BalanceOf<T>,
		},
		ReplacePeriodChange {
			period: T::BlockNumber,
		},
		ReplaceMinimumTransferAmountUpdate {
			new_minimum_amount: BalanceOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Replace requires non-zero increase.
		ReplaceAmountZero,
		/// Replace amount is too small.
		AmountBelowDustAmount,
		/// No replace request found.
		NoPendingRequest,
		/// Unexpected vault account.
		UnauthorizedVault,
		/// Cannot replace self.
		ReplaceSelfNotAllowed,
		/// Cannot replace with nominated collateral.
		VaultHasEnabledNomination,
		/// Replace request has not expired.
		ReplacePeriodNotExpired,
		/// Replace request already completed.
		ReplaceCompleted,
		/// Replace request already cancelled.
		ReplaceCancelled,
		/// Replace request not found.
		ReplaceIdNotFound,
		/// Vault cannot replace different currency.
		InvalidWrappedCurrency,
		/// Invalid payment amount
		InvalidPaymentAmount,
	}

	/// Vaults create replace requests to transfer locked collateral.
	/// This mapping provides access from a unique hash to a `ReplaceRequest`.
	#[pallet::storage]
	pub(super) type ReplaceRequests<T: Config> =
		StorageMap<_, Blake2_128Concat, H256, DefaultReplaceRequest<T>, OptionQuery>;

	/// The time difference in number of blocks between when a replace request is created
	/// and required completion time by a vault. The replace period has an upper limit
	/// to prevent griefing of vault collateral.
	#[pallet::storage]
	#[pallet::getter(fn replace_period)]
	pub(super) type ReplacePeriod<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	/// The minimum amount of wrapped assets that is accepted for replace requests
	#[pallet::storage]
	#[pallet::getter(fn replace_minimum_transfer_amount)]
	pub(super) type ReplaceMinimumTransferAmount<T: Config> =
		StorageValue<_, BalanceOf<T>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub replace_period: T::BlockNumber,
		pub replace_minimum_transfer_amount: BalanceOf<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				replace_period: Default::default(),
				replace_minimum_transfer_amount: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			ReplacePeriod::<T>::put(self.replace_period);
			ReplaceMinimumTransferAmount::<T>::put(self.replace_minimum_transfer_amount);
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	// The pallet's dispatchable functions.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Request the replacement of a new vault ownership
		///
		/// # Arguments
		///
		/// * `origin` - sender of the transaction
		/// * `amount` - amount of issued tokens
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config >::WeightInfo::request_replace())]
		#[transactional]
		pub fn request_replace(
			origin: OriginFor<T>,
			currency_pair: DefaultVaultCurrencyPair<T>,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let old_vault = VaultId::new(
				ensure_signed(origin)?,
				currency_pair.collateral,
				currency_pair.wrapped,
			);
			Self::_request_replace(old_vault, amount)?;
			Ok(().into())
		}

		/// Withdraw a request of vault replacement
		///
		/// # Arguments
		///
		/// * `origin` - sender of the transaction: the old vault
		/// * `amount` - amount of tokens to be withdrawn from being replaced
		#[pallet::call_index(1)]
		#[pallet::weight(< T as Config >::WeightInfo::withdraw_replace())]
		#[transactional]
		pub fn withdraw_replace(
			origin: OriginFor<T>,
			currency_pair: DefaultVaultCurrencyPair<T>,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let old_vault = VaultId::new(
				ensure_signed(origin)?,
				currency_pair.collateral,
				currency_pair.wrapped,
			);
			Self::_withdraw_replace_request(old_vault, amount)?;
			Ok(().into())
		}

		/// Accept request of vault replacement
		///
		/// # Arguments
		///
		/// * `origin` - the initiator of the transaction: the new vault
		/// * `currency_pair` - currency_pair of the new vault
		/// * `amount` - amount of tokens to be replaced
		/// * `collateral` - the collateral provided by the new vault to match the replace request
		///   (for backing the transferred tokens)
		/// * `stellar_address` - the address that old-vault should transfer the wrapped asset to
		#[pallet::call_index(2)]
		#[pallet::weight(< T as Config >::WeightInfo::accept_replace())]
		#[transactional]
		pub fn accept_replace(
			origin: OriginFor<T>,
			currency_pair: DefaultVaultCurrencyPair<T>,
			old_vault: DefaultVaultId<T>,
			#[pallet::compact] amount: BalanceOf<T>,
			#[pallet::compact] collateral: BalanceOf<T>,
			stellar_address: StellarPublicKeyRaw,
		) -> DispatchResultWithPostInfo {
			let new_vault = VaultId::new(
				ensure_signed(origin)?,
				currency_pair.collateral,
				currency_pair.wrapped,
			);
			Self::_accept_replace(old_vault, new_vault, amount, collateral, stellar_address)?;
			Ok(().into())
		}

		/// Execute vault replacement
		///
		/// # Arguments
		///
		/// * `origin` - sender of the transaction: the new vault
		/// * `replace_id` - the ID of the replacement request
		#[pallet::call_index(3)]
		#[pallet::weight(< T as Config >::WeightInfo::execute_replace())]
		#[transactional]
		pub fn execute_replace(
			origin: OriginFor<T>,
			replace_id: H256,
			transaction_envelope_xdr_encoded: Vec<u8>,
			externalized_envelopes_xdr_encoded: Vec<u8>,
			transaction_set_xdr_encoded: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;
			Self::_execute_replace(
				replace_id,
				transaction_envelope_xdr_encoded,
				externalized_envelopes_xdr_encoded,
				transaction_set_xdr_encoded,
			)?;
			Ok(().into())
		}

		/// Cancel vault replacement
		///
		/// # Arguments
		///
		/// * `origin` - sender of the transaction: anyone
		/// * `replace_id` - the ID of the replacement request
		#[pallet::call_index(4)]
		#[pallet::weight(< T as Config >::WeightInfo::cancel_replace())]
		#[transactional]
		pub fn cancel_replace(
			origin: OriginFor<T>,
			replace_id: H256,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;
			Self::_cancel_replace(replace_id)?;
			Ok(().into())
		}

		/// Set the default replace period for tx verification.
		///
		/// # Arguments
		///
		/// * `origin` - the dispatch origin of this call (must be _Root_)
		/// * `period` - default period for new requests
		///
		/// # Weight: `O(1)`
		#[pallet::call_index(5)]
		#[pallet::weight(< T as Config >::WeightInfo::set_replace_period())]
		#[transactional]
		pub fn set_replace_period(
			origin: OriginFor<T>,
			period: T::BlockNumber,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			<ReplacePeriod<T>>::set(period);
			Self::deposit_event(Event::ReplacePeriodChange { period });
			Ok(().into())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::minimum_transfer_amount_update())]
		#[transactional]
		pub fn minimum_transfer_amount_update(
			origin: OriginFor<T>,
			new_minimum_amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			ReplaceMinimumTransferAmount::<T>::set(new_minimum_amount);
			Self::deposit_event(Event::ReplaceMinimumTransferAmountUpdate { new_minimum_amount });
			Ok(().into())
		}
	}
}

// "Internal" functions, callable by code.
#[cfg_attr(test, mockable)]
impl<T: Config> Pallet<T> {
	fn _request_replace(vault_id: DefaultVaultId<T>, amount: BalanceOf<T>) -> DispatchResult {
		// check vault is not banned
		ext::vault_registry::ensure_not_banned::<T>(&vault_id)?;

		let amount_to_replace = Amount::new(amount, vault_id.wrapped_currency());
		// We ensure that the amount requested is compatible with the target chain (ie. it has a
		// specific amount of trailing zeros)
		amount_to_replace.ensure_is_compatible_with_target_chain()?;

		ensure!(
			!ext::nomination::is_nominatable::<T>(&vault_id)?,
			Error::<T>::VaultHasEnabledNomination
		);

		let requestable_tokens =
			ext::vault_registry::requestable_to_be_replaced_tokens::<T>(&vault_id)?;
		let to_be_replaced_increase = amount_to_replace.min(&requestable_tokens)?;

		ensure!(!to_be_replaced_increase.is_zero(), Error::<T>::ReplaceAmountZero);

		// increase to-be-replaced tokens. This will fail if the vault does not have enough tokens
		// available
		let total_to_be_replaced = ext::vault_registry::try_increase_to_be_replaced_tokens::<T>(
			&vault_id,
			&to_be_replaced_increase,
		)?;

		// check that total-to-be-replaced is above the minimum
		ensure!(
			total_to_be_replaced.ge(&Self::minimum_transfer_amount(vault_id.wrapped_currency()))?,
			Error::<T>::AmountBelowDustAmount
		);

		// get the griefing collateral increase
		let griefing_collateral = ext::fee::get_replace_griefing_collateral::<T>(
			&to_be_replaced_increase.convert_to(T::GetGriefingCollateralCurrencyId::get())?,
		)?;

		// Lock the oldVault’s griefing collateral
		ext::vault_registry::transfer_funds(
			CurrencySource::FreeBalance(vault_id.account_id.clone()),
			CurrencySource::AvailableReplaceCollateral(vault_id.clone()),
			&griefing_collateral,
		)?;

		// Emit RequestReplace event
		Self::deposit_event(Event::<T>::RequestReplace {
			old_vault_id: vault_id,
			amount: to_be_replaced_increase.amount(),
			asset: to_be_replaced_increase.currency(),
			griefing_collateral: griefing_collateral.amount(),
		});
		Ok(())
	}

	fn _withdraw_replace_request(
		vault_id: DefaultVaultId<T>,
		amount: BalanceOf<T>,
	) -> Result<(), DispatchError> {
		let amount = Amount::new(amount, vault_id.wrapped_currency());
		// decrease to-be-replaced tokens, so that the vault is free to use its issued tokens again.
		let (withdrawn_tokens, to_withdraw_collateral) =
			ext::vault_registry::withdraw_replace_request::<T>(&vault_id, &amount)?;

		if withdrawn_tokens.is_zero() {
			return Err(Error::<T>::NoPendingRequest.into())
		}

		// Emit WithdrawReplaceRequest event.
		Self::deposit_event(Event::<T>::WithdrawReplace {
			old_vault_id: vault_id,
			withdrawn_tokens: withdrawn_tokens.amount(),
			asset: withdrawn_tokens.currency(),
			withdrawn_griefing_collateral: to_withdraw_collateral.amount(),
		});
		Ok(())
	}

	fn accept_replace_tokens(
		old_vault_id: &DefaultVaultId<T>,
		new_vault_id: &DefaultVaultId<T>,
		redeemable_tokens: &Amount<T>,
	) -> DispatchResult {
		// increase old-vault's to-be-redeemed tokens - this should never fail
		ext::vault_registry::try_increase_to_be_redeemed_tokens::<T>(
			old_vault_id,
			redeemable_tokens,
		)?;

		// increase new-vault's to-be-issued tokens - this will fail if there is insufficient
		// collateral
		ext::vault_registry::try_increase_to_be_issued_tokens::<T>(
			new_vault_id,
			redeemable_tokens,
		)?;

		Ok(())
	}

	fn _accept_replace(
		old_vault_id: DefaultVaultId<T>,
		new_vault_id: DefaultVaultId<T>,
		amount: BalanceOf<T>,
		collateral: BalanceOf<T>,
		stellar_address: StellarPublicKeyRaw,
	) -> Result<(), DispatchError> {
		let new_vault_currency_id = new_vault_id.collateral_currency();
		let replace_amount = Amount::new(amount, old_vault_id.wrapped_currency());
		let collateral = Amount::new(collateral, new_vault_currency_id);

		// don't allow vaults to replace themselves
		ensure!(old_vault_id != new_vault_id, Error::<T>::ReplaceSelfNotAllowed);

		// probably this check is not strictly required, but it's better to give an
		// explicit error rather than insufficient balance
		ensure!(
			old_vault_id.wrapped_currency() == new_vault_id.wrapped_currency(),
			Error::<T>::InvalidWrappedCurrency
		);

		// Check that new vault is not currently banned
		ext::vault_registry::ensure_not_banned::<T>(&new_vault_id)?;

		// decrease old-vault's to-be-replaced tokens
		let (redeemable_tokens, griefing_collateral) =
			ext::vault_registry::decrease_to_be_replaced_tokens::<T>(
				&old_vault_id,
				&replace_amount,
			)?;

		// check replace_amount is above the minimum
		ensure!(
			redeemable_tokens
				.ge(&Self::minimum_transfer_amount(old_vault_id.wrapped_currency()))?,
			Error::<T>::AmountBelowDustAmount
		);

		// Calculate and lock the new-vault's additional collateral
		let actual_new_vault_collateral = ext::vault_registry::calculate_collateral::<T>(
			&collateral,
			&redeemable_tokens,
			&replace_amount,
		)?;

		ext::vault_registry::try_deposit_collateral::<T>(
			&new_vault_id,
			&actual_new_vault_collateral,
		)?;

		Self::accept_replace_tokens(&old_vault_id, &new_vault_id, &redeemable_tokens)?;

		ext::vault_registry::transfer_funds(
			CurrencySource::AvailableReplaceCollateral(old_vault_id.clone()),
			CurrencySource::ActiveReplaceCollateral(old_vault_id.clone()),
			&griefing_collateral,
		)?;

		let replace_id = ext::security::get_secure_id::<T>();

		let replace = ReplaceRequest {
			old_vault: old_vault_id,
			new_vault: new_vault_id,
			accept_time: ext::security::active_block_number::<T>(),
			collateral: actual_new_vault_collateral.amount(),
			stellar_address,
			griefing_collateral: griefing_collateral.amount(),
			amount: redeemable_tokens.amount(),
			asset: redeemable_tokens.currency(),
			period: Self::replace_period(),
			status: ReplaceRequestStatus::Pending,
		};

		Self::insert_replace_request(&replace_id, &replace);

		// Emit AcceptReplace event
		Self::deposit_event(Event::<T>::AcceptReplace {
			replace_id,
			old_vault_id: replace.old_vault,
			new_vault_id: replace.new_vault,
			amount: replace.amount,
			asset: replace.asset,
			collateral: replace.collateral,
			stellar_address: replace.stellar_address,
		});

		Ok(())
	}

	fn _execute_replace(
		replace_id: H256,
		transaction_envelope_xdr_encoded: Vec<u8>,
		externalized_envelopes_xdr_encoded: Vec<u8>,
		transaction_set_xdr_encoded: Vec<u8>,
	) -> DispatchResult {
		// retrieve the replace request using the id parameter
		// we can still execute cancelled requests
		let replace = Self::get_open_or_cancelled_replace_request(&replace_id)?;

		let griefing_collateral: Amount<T> = replace.griefing_collateral();
		let amount = replace.amount();
		let collateral = replace.collateral()?;

		// NOTE: anyone can call this method provided the proof is correct
		let new_vault_id = replace.new_vault;
		let old_vault_id = replace.old_vault;

		let transaction_envelope = ext::stellar_relay::construct_from_raw_encoded_xdr::<
			T,
			TransactionEnvelope,
		>(&transaction_envelope_xdr_encoded)?;

		let envelopes = ext::stellar_relay::construct_from_raw_encoded_xdr::<
			T,
			UnlimitedVarArray<ScpEnvelope>,
		>(&externalized_envelopes_xdr_encoded)?;

		let transaction_set = ext::stellar_relay::construct_from_raw_encoded_xdr::<
			T,
			TransactionSet,
		>(&transaction_set_xdr_encoded)?;

		// Check that the transaction includes the expected memo to mitigate replay attacks
		ext::stellar_relay::ensure_transaction_memo_matches_hash::<T>(
			&transaction_envelope,
			&replace_id,
		)?;

		// Verify that the transaction is valid
		ext::stellar_relay::validate_stellar_transaction::<T>(
			&transaction_envelope,
			&envelopes,
			&transaction_set,
		)
		.map_err(|e| {
			log::error!(
				"failed to validate transaction of replace id: {} with transaction envelope: {transaction_envelope:?}",
				hex::encode(replace_id.as_bytes())
			);

			e
		})?;

		let paid_amount: Amount<T> = ext::currency::get_amount_from_transaction_envelope::<T>(
			&transaction_envelope,
			replace.stellar_address,
			replace.asset,
		)?;

		// Check that the transaction contains a payment with at least the expected amount
		ensure!(paid_amount.ge(&amount)?, Error::<T>::InvalidPaymentAmount);

		// only return griefing collateral if not already slashed
		let collateral = match replace.status {
			ReplaceRequestStatus::Pending => {
				// give old-vault the griefing collateral
				ext::vault_registry::transfer_funds(
					CurrencySource::ActiveReplaceCollateral(old_vault_id.clone()),
					CurrencySource::FreeBalance(old_vault_id.account_id.clone()),
					&griefing_collateral,
				)?;
				// NOTE: this is just the additional collateral already locked on accept
				// it is only used in the ReplaceTokens event
				collateral
			},
			ReplaceRequestStatus::Cancelled => {
				// we need to re-accept first, this will check that the vault is over the secure
				// threshold
				Self::accept_replace_tokens(&old_vault_id, &new_vault_id, &amount)?;
				// no additional collateral locked for this
				Amount::zero(collateral.currency())
			},
			ReplaceRequestStatus::Completed => {
				// we never enter this branch as completed requests are filtered
				return Err(Error::<T>::ReplaceCompleted.into())
			},
		};

		// decrease old-vault's issued & to-be-redeemed tokens, and
		// change new-vault's to-be-issued tokens to issued tokens
		ext::vault_registry::replace_tokens::<T>(
			&old_vault_id,
			&new_vault_id,
			&amount,
			&collateral,
		)?;

		// Emit ExecuteReplace event.
		Self::deposit_event(Event::<T>::ExecuteReplace { replace_id, old_vault_id, new_vault_id });

		// Remove replace request
		Self::set_replace_status(&replace_id, ReplaceRequestStatus::Completed);
		Ok(())
	}

	fn _cancel_replace(replace_id: H256) -> Result<(), DispatchError> {
		// Retrieve the ReplaceRequest as per the replaceId parameter from Vaults in the
		// VaultRegistry
		let replace = Self::get_open_replace_request(&replace_id)?;

		let griefing_collateral: Amount<T> = replace.griefing_collateral();
		let amount = replace.amount();
		let collateral = replace.collateral()?;

		// only cancellable after the request has expired
		ensure!(
			ext::security::parachain_block_expired::<T>(
				replace.accept_time,
				Self::replace_period().max(replace.period)
			)?,
			Error::<T>::ReplacePeriodNotExpired
		);

		let new_vault_id = replace.new_vault;

		// decrease old-vault's to-be-redeemed tokens, and
		// decrease new-vault's to-be-issued tokens
		ext::vault_registry::cancel_replace_tokens::<T>(
			&replace.old_vault,
			&new_vault_id,
			&amount,
		)?;

		// slash old-vault's griefing collateral
		ext::vault_registry::transfer_funds::<T>(
			CurrencySource::ActiveReplaceCollateral(replace.old_vault.clone()),
			CurrencySource::FreeBalance(new_vault_id.account_id.clone()),
			&griefing_collateral,
		)?;

		// if the new_vault locked additional collateral especially for this replace,
		// release it if it does not cause them to be undercollateralized
		if !ext::vault_registry::is_vault_liquidated::<T>(&new_vault_id)? &&
			ext::vault_registry::is_allowed_to_withdraw_collateral::<T>(
				&new_vault_id,
				&collateral,
			)? {
			ext::vault_registry::force_withdraw_collateral::<T>(&new_vault_id, &collateral)?;
		}

		// Remove the ReplaceRequest from ReplaceRequests
		Self::set_replace_status(&replace_id, ReplaceRequestStatus::Cancelled);

		// Emit CancelReplace event.
		Self::deposit_event(Event::<T>::CancelReplace {
			replace_id,
			new_vault_id,
			old_vault_id: replace.old_vault,
			griefing_collateral: replace.griefing_collateral,
		});
		Ok(())
	}

	/// Fetch all replace requests from the specified vault.
	///
	/// # Arguments
	///
	/// * `account_id` - user account id
	pub fn get_replace_requests_for_old_vault(vault_id: T::AccountId) -> Vec<H256> {
		<ReplaceRequests<T>>::iter()
			.filter(|(_, request)| request.old_vault.account_id == vault_id)
			.map(|(key, _)| key)
			.collect::<Vec<_>>()
	}

	/// Fetch all replace requests to the specified vault.
	///
	/// # Arguments
	///
	/// * `account_id` - user account id
	pub fn get_replace_requests_for_new_vault(vault_id: T::AccountId) -> Vec<H256> {
		<ReplaceRequests<T>>::iter()
			.filter(|(_, request)| request.new_vault.account_id == vault_id)
			.map(|(key, _)| key)
			.collect::<Vec<_>>()
	}

	/// Get a replace request by id. Completed or cancelled requests are not returned.
	pub fn get_open_replace_request(
		replace_id: &H256,
	) -> Result<DefaultReplaceRequest<T>, DispatchError> {
		let request =
			ReplaceRequests::<T>::try_get(replace_id).or(Err(Error::<T>::ReplaceIdNotFound))?;

		// NOTE: temporary workaround until we delete
		match request.status {
			ReplaceRequestStatus::Pending => Ok(request),
			ReplaceRequestStatus::Completed => Err(Error::<T>::ReplaceCompleted.into()),
			ReplaceRequestStatus::Cancelled => Err(Error::<T>::ReplaceCancelled.into()),
		}
	}

	/// Get a open or completed replace request by id. Cancelled requests are not returned.
	pub fn get_open_or_completed_replace_request(
		id: &H256,
	) -> Result<DefaultReplaceRequest<T>, DispatchError> {
		let request = <ReplaceRequests<T>>::get(id).ok_or(Error::<T>::ReplaceIdNotFound)?;
		match request.status {
			ReplaceRequestStatus::Pending | ReplaceRequestStatus::Completed => Ok(request),
			ReplaceRequestStatus::Cancelled => Err(Error::<T>::ReplaceCancelled.into()),
		}
	}

	/// Get a open or cancelled replace request by id. Completed requests are not returned.
	pub fn get_open_or_cancelled_replace_request(
		id: &H256,
	) -> Result<DefaultReplaceRequest<T>, DispatchError> {
		let request = <ReplaceRequests<T>>::get(id).ok_or(Error::<T>::ReplaceIdNotFound)?;
		match request.status {
			ReplaceRequestStatus::Pending | ReplaceRequestStatus::Cancelled => Ok(request),
			ReplaceRequestStatus::Completed => Err(Error::<T>::ReplaceCompleted.into()),
		}
	}

	fn insert_replace_request(key: &H256, value: &DefaultReplaceRequest<T>) {
		<ReplaceRequests<T>>::insert(key, value)
	}

	fn set_replace_status(key: &H256, status: ReplaceRequestStatus) {
		<ReplaceRequests<T>>::mutate_exists(key, |request| {
			*request = request
				.clone()
				.map(|request| DefaultReplaceRequest::<T> { status: status.clone(), ..request });
		});
	}

	pub fn minimum_transfer_amount(currency_id: CurrencyId<T>) -> Amount<T> {
		Amount::new(ReplaceMinimumTransferAmount::<T>::get(), currency_id)
	}
}
