//! # Redeem Pallet
//! Based on the [specification](https://spec.interlay.io/spec/redeem.html).

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
extern crate mocktopus;

#[cfg(feature = "std")]
use std::str::FromStr;

use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	ensure, log, transactional,
};
#[cfg(test)]
use mocktopus::macros::mockable;

use sp_core::H256;
use sp_runtime::traits::{CheckedDiv, Saturating, Zero};
use sp_std::{convert::TryInto, vec::Vec};
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
use vault_registry::{
	types::{CurrencyId, DefaultVaultCurrencyPair},
	CurrencySource,
};

use crate::types::{BalanceOf, RedeemRequestExt};
#[doc(inline)]
pub use crate::types::{DefaultRedeemRequest, RedeemRequest, RedeemRequestStatus};

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod default_weights;
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod ext;
pub mod types;

const SECONDS_PER_BLOCK: u32 = 12;
const MINUTE: u32 = 60; // in seconds
const HOUR: u32 = MINUTE * 60;
const DAY: u32 = HOUR * 24;

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
		+ fee::Config<UnsignedInner = BalanceOf<Self>>
	{
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		RequestRedeem {
			redeem_id: H256,
			redeemer: T::AccountId,
			vault_id: DefaultVaultId<T>,
			amount: BalanceOf<T>,
			asset: CurrencyId<T>,
			fee: BalanceOf<T>,
			premium: BalanceOf<T>,
			stellar_address: StellarPublicKeyRaw,
			transfer_fee: BalanceOf<T>,
		},
		LiquidationRedeem {
			redeemer: T::AccountId,
			amount: BalanceOf<T>,
			asset: CurrencyId<T>,
		},
		ExecuteRedeem {
			redeem_id: H256,
			redeemer: T::AccountId,
			vault_id: DefaultVaultId<T>,
			amount: BalanceOf<T>,
			asset: CurrencyId<T>,
			fee: BalanceOf<T>,
			transfer_fee: BalanceOf<T>,
		},
		CancelRedeem {
			redeem_id: H256,
			redeemer: T::AccountId,
			vault_id: DefaultVaultId<T>,
			slashed_amount: BalanceOf<T>,
			status: RedeemRequestStatus,
		},
		MintTokensForReimbursedRedeem {
			redeem_id: H256,
			vault_id: DefaultVaultId<T>,
			amount: BalanceOf<T>,
		},
		RedeemPeriodChange {
			period: T::BlockNumber,
		},
		SelfRedeem {
			vault_id: DefaultVaultId<T>,
			amount: BalanceOf<T>,
			fee: BalanceOf<T>,
		},
		RateLimitUpdate {
			limit_volume_amount: Option<BalanceOf<T>>,
			limit_volume_currency_id: T::CurrencyId,
			interval_length: T::BlockNumber,
		},
		RedeemMinimumTransferAmountUpdate {
			new_minimum_amount: BalanceOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Account has insufficient balance.
		AmountExceedsUserBalance,
		/// Unexpected redeem account.
		UnauthorizedRedeemer,
		/// Unexpected vault account.
		UnauthorizedVault,
		/// Redeem request has not expired.
		TimeNotExpired,
		/// Redeem request already cancelled.
		RedeemCancelled,
		/// Redeem request already completed.
		RedeemCompleted,
		/// Redeem request not found.
		RedeemIdNotFound,
		/// Unable to convert value.
		TryIntoIntError,
		/// Redeem amount is too small.
		AmountBelowMinimumTransferAmount,
		/// Exceeds the volume limit for an issue request.
		ExceedLimitVolumeForIssueRequest,
		/// Invalid payment amount
		InvalidPaymentAmount,
	}

	/// The time difference in number of blocks between a redeem request is created and required
	/// completion time by a vault. The redeem period has an upper limit to ensure the user gets
	/// their Stellar assets in time and to potentially punish a vault for inactivity or stealing
	/// Stellar assets.
	#[pallet::storage]
	#[pallet::getter(fn redeem_period)]
	pub(super) type RedeemPeriod<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	/// Users create redeem requests to receive stellar assets in return for their previously issued
	/// tokens. This mapping provides access from a unique hash redeemId to a Redeem struct.
	#[pallet::storage]
	#[pallet::getter(fn redeem_requests)]
	pub(super) type RedeemRequests<T: Config> =
		StorageMap<_, Blake2_128Concat, H256, DefaultRedeemRequest<T>, OptionQuery>;

	/// The minimum amount of wrapped assets that is accepted for redeem requests
	#[pallet::storage]
	#[pallet::getter(fn redeem_minimum_transfer_amount)]
	pub(super) type RedeemMinimumTransferAmount<T: Config> =
		StorageValue<_, BalanceOf<T>, ValueQuery>;

	#[pallet::storage]
	pub(super) type LimitVolumeAmount<T: Config> =
		StorageValue<_, Option<BalanceOf<T>>, ValueQuery>;

	/// CurrencyID that represents the currency in which the volume limit is measured, eg DOT, USDC
	/// or PEN
	#[pallet::storage]
	pub(super) type LimitVolumeCurrencyId<T: Config> = StorageValue<_, T::CurrencyId, ValueQuery>;

	#[pallet::storage]
	pub(super) type CurrentVolumeAmount<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	/// Represent interval define regular 24 hour intervals (every 24 * 3600 / 12 blocks)
	#[pallet::storage]
	pub(super) type IntervalLength<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	/// Represent current interval current_block_number / IntervalLength
	#[pallet::storage]
	pub(super) type LastIntervalIndex<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	#[pallet::storage]
	pub(super) type CancelledRedeemAmount<T: Config> =
		StorageMap<_, Blake2_128Concat, H256, (BalanceOf<T>, T::CurrencyId), OptionQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub redeem_period: T::BlockNumber,
		pub redeem_minimum_transfer_amount: BalanceOf<T>,
		pub limit_volume_amount: Option<BalanceOf<T>>,
		pub limit_volume_currency_id: T::CurrencyId,
		pub current_volume_amount: BalanceOf<T>,
		pub interval_length: T::BlockNumber,
		pub last_interval_index: T::BlockNumber,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				redeem_period: Default::default(),
				redeem_minimum_transfer_amount: Default::default(),
				limit_volume_amount: None,
				limit_volume_currency_id: T::CurrencyId::default(),
				current_volume_amount: BalanceOf::<T>::zero(),
				interval_length: T::BlockNumber::from_str(&(DAY / SECONDS_PER_BLOCK).to_string())
					.unwrap_or_default(),
				last_interval_index: T::BlockNumber::zero(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			RedeemPeriod::<T>::put(self.redeem_period);
			RedeemMinimumTransferAmount::<T>::put(self.redeem_minimum_transfer_amount);
			LimitVolumeAmount::<T>::put(self.limit_volume_amount);
			LimitVolumeCurrencyId::<T>::put(self.limit_volume_currency_id);
			CurrentVolumeAmount::<T>::put(self.current_volume_amount);
			IntervalLength::<T>::put(self.interval_length);
			LastIntervalIndex::<T>::put(self.last_interval_index);
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	// The pallet's dispatchable functions.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Initializes a request to burn issued tokens against a Vault with sufficient tokens. It
		/// will also ensure that the Parachain status is RUNNING.
		///
		/// # Arguments
		///
		/// * `origin` - sender of the transaction
		/// * `amount_wrapped` - amount of tokens to redeem
		/// * `asset` - the asset to redeem
		/// * `stellar_address` - the address to receive assets on Stellar
		/// * `vault_id` - address of the vault
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::request_redeem())]
		#[transactional]
		pub fn request_redeem(
			origin: OriginFor<T>,
			#[pallet::compact] amount_wrapped: BalanceOf<T>,
			stellar_address: StellarPublicKeyRaw,
			vault_id: DefaultVaultId<T>,
		) -> DispatchResultWithPostInfo {
			let redeemer = ensure_signed(origin)?;
			Self::_request_redeem(redeemer, amount_wrapped, stellar_address, vault_id)?;
			Ok(().into())
		}

		/// When a Vault is liquidated, its collateral is slashed up to 150% of the liquidated
		/// value. To re-establish the physical 1:1 peg, the bridge allows users to burn issued
		/// tokens in return for collateral at a premium rate.
		///
		/// # Arguments
		///
		/// * `origin` - sender of the transaction
		/// * `collateral_currency` - currency to be received
		/// * `wrapped_currency` - currency of the wrapped token to burn
		/// * `amount_wrapped` - amount of issued tokens to burn
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::liquidation_redeem())]
		#[transactional]
		pub fn liquidation_redeem(
			origin: OriginFor<T>,
			currencies: DefaultVaultCurrencyPair<T>,
			#[pallet::compact] amount_wrapped: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let redeemer = ensure_signed(origin)?;
			Self::_liquidation_redeem(redeemer, currencies, amount_wrapped)?;
			Ok(().into())
		}

		/// A Vault calls this function after receiving an RequestRedeem event with their public
		/// key. Before calling the function, the Vault transfers the specific amount of Stellar
		/// assets to the Stellar address given in the original redeem request. The Vault completes
		/// the redeem with this function.
		///
		/// # Arguments
		///
		/// * `origin` - anyone executing this redeem request
		/// * `redeem_id` - identifier of redeem request as output from request_redeem
		/// * `transaction_envelope_xdr_encoded` - the XDR representation of the transaction
		///   envelope
		/// * `externalized_envelopes_encoded` - the XDR representation of the externalized
		///   envelopes
		/// * `transaction_set_encoded` - the XDR representation of the transaction set
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::execute_redeem())]
		#[transactional]
		pub fn execute_redeem(
			origin: OriginFor<T>,
			redeem_id: H256,
			transaction_envelope_xdr_encoded: Vec<u8>,
			externalized_envelopes_encoded: Vec<u8>,
			transaction_set_encoded: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;
			Self::_execute_redeem(
				redeem_id,
				transaction_envelope_xdr_encoded,
				externalized_envelopes_encoded,
				transaction_set_encoded,
			)?;

			// Don't take tx fees on success. If the vault had to pay for this function, it would
			// have been vulnerable to a griefing attack where users would redeem amounts just
			// above the minimum transfer value.
			Ok(Pays::No.into())
		}

		/// If a redeem request is not completed on time, the redeem request can be cancelled.
		/// The user that initially requested the redeem process calls this function to obtain
		/// the Vault’s collateral as compensation for not transferring the Stellar assets back to
		/// their address.
		///
		/// # Arguments
		///
		/// * `origin` - sender of the transaction
		/// * `redeem_id` - identifier of redeem request as output from request_redeem
		/// * `reimburse` - specifying if the user wishes to be reimbursed in collateral
		/// and slash the Vault, or wishes to keep the tokens (and retry
		/// Redeem with another Vault)
		#[pallet::call_index(3)]
		#[pallet::weight(if *reimburse { <T as Config>::WeightInfo::cancel_redeem_reimburse() } else { <T as Config>::WeightInfo::cancel_redeem_retry() })]
		#[transactional]
		pub fn cancel_redeem(
			origin: OriginFor<T>,
			redeem_id: H256,
			reimburse: bool,
		) -> DispatchResultWithPostInfo {
			let redeemer = ensure_signed(origin)?;
			Self::_cancel_redeem(redeemer, redeem_id, reimburse)?;
			Ok(().into())
		}

		/// Set the default redeem period for tx verification.
		///
		/// # Arguments
		///
		/// * `origin` - the dispatch origin of this call (must be _Root_)
		/// * `period` - default period for new requests
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::set_redeem_period())]
		#[transactional]
		pub fn set_redeem_period(
			origin: OriginFor<T>,
			period: T::BlockNumber,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			<RedeemPeriod<T>>::set(period);
			Self::deposit_event(Event::RedeemPeriodChange { period });
			Ok(().into())
		}

		/// Mint tokens for a redeem that was cancelled with reimburse=true. This is
		/// only possible if at the time of the cancel_redeem, the vault did not have
		/// sufficient collateral after being slashed to back the tokens that the user
		/// used to hold.
		///
		/// # Arguments
		///
		/// * `origin` - the dispatch origin of this call (must be _Root_)
		/// * `redeem_id` - identifier of redeem request as output from request_redeem
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::mint_tokens_for_reimbursed_redeem())]
		#[transactional]
		pub fn mint_tokens_for_reimbursed_redeem(
			origin: OriginFor<T>,
			currency_pair: DefaultVaultCurrencyPair<T>,
			redeem_id: H256,
		) -> DispatchResultWithPostInfo {
			let vault_id = VaultId::new(
				ensure_signed(origin)?,
				currency_pair.collateral,
				currency_pair.wrapped,
			);
			Self::_mint_tokens_for_reimbursed_redeem(vault_id, redeem_id)?;
			Ok(().into())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::self_redeem())]
		#[transactional]
		pub fn self_redeem(
			origin: OriginFor<T>,
			currency_pair: DefaultVaultCurrencyPair<T>,
			amount_wrapped: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let account_id = ensure_signed(origin)?;
			let vault_id =
				VaultId::new(account_id, currency_pair.collateral, currency_pair.wrapped);
			let amount_wrapped = Amount::new(amount_wrapped, vault_id.wrapped_currency());

			self_redeem::execute::<T>(vault_id, amount_wrapped)?;

			Ok(().into())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::rate_limit_update())]
		#[transactional]
		pub fn rate_limit_update(
			origin: OriginFor<T>,
			limit_volume_amount: Option<BalanceOf<T>>,
			limit_volume_currency_id: T::CurrencyId,
			interval_length: T::BlockNumber,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			Self::_rate_limit_update(
				limit_volume_amount,
				limit_volume_currency_id,
				interval_length,
			);
			Self::deposit_event(Event::RateLimitUpdate {
				limit_volume_amount,
				limit_volume_currency_id,
				interval_length,
			});
			Ok(().into())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(<T as Config>::WeightInfo::minimum_transfer_amount_update())]
		#[transactional]
		pub fn minimum_transfer_amount_update(
			origin: OriginFor<T>,
			new_minimum_amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			RedeemMinimumTransferAmount::<T>::set(new_minimum_amount);
			Self::deposit_event(Event::RedeemMinimumTransferAmountUpdate { new_minimum_amount });
			Ok(().into())
		}
	}
}

mod self_redeem {
	use super::*;

	pub(crate) fn execute<T: Config>(
		vault_id: DefaultVaultId<T>,
		amount_wrapped: Amount<T>,
	) -> DispatchResult {
		// ensure that vault is not liquidated and not banned
		ext::vault_registry::ensure_not_banned::<T>(&vault_id)?;

		// for self-redeem, dustAmount is effectively 1 satoshi
		ensure!(!amount_wrapped.is_zero(), Error::<T>::AmountBelowMinimumTransferAmount);

		let (fees, consumed_issued_tokens) =
			calculate_token_amounts::<T>(&vault_id, &amount_wrapped)?;

		take_user_tokens::<T>(&vault_id.account_id, &consumed_issued_tokens, &fees)?;

		update_vault_tokens::<T>(&vault_id, &consumed_issued_tokens)?;

		Pallet::<T>::deposit_event(Event::<T>::SelfRedeem {
			vault_id,
			amount: consumed_issued_tokens.amount(),
			fee: fees.amount(),
		});

		Ok(())
	}

	fn calculate_token_amounts<T: Config>(
		vault_id: &DefaultVaultId<T>,
		requested_redeem_amount: &Amount<T>,
	) -> Result<(Amount<T>, Amount<T>), DispatchError> {
		let redeemable_tokens = ext::vault_registry::get_free_redeemable_tokens(vault_id)?;

		let fees = if redeemable_tokens.eq(requested_redeem_amount)? {
			Amount::zero(vault_id.wrapped_currency())
		} else {
			ext::fee::get_redeem_fee::<T>(requested_redeem_amount)?
		};

		let consumed_issued_tokens = requested_redeem_amount.checked_sub(&fees)?;

		Ok((fees, consumed_issued_tokens))
	}

	fn take_user_tokens<T: Config>(
		account_id: &T::AccountId,
		consumed_issued_tokens: &Amount<T>,
		fees: &Amount<T>,
	) -> DispatchResult {
		// burn the tokens that the vault no longer is backing
		consumed_issued_tokens
			.lock_on(account_id)
			.map_err(|_| Error::<T>::AmountExceedsUserBalance)?;
		consumed_issued_tokens.burn_from(account_id)?;

		// transfer fees to pool
		fees.transfer(account_id, &ext::fee::fee_pool_account_id::<T>())
			.map_err(|_| Error::<T>::AmountExceedsUserBalance)?;
		ext::fee::distribute_rewards::<T>(fees)?;

		Ok(())
	}

	fn update_vault_tokens<T: Config>(
		vault_id: &DefaultVaultId<T>,
		consumed_issued_tokens: &Amount<T>,
	) -> DispatchResult {
		ext::vault_registry::try_increase_to_be_redeemed_tokens::<T>(
			vault_id,
			consumed_issued_tokens,
		)?;
		ext::vault_registry::redeem_tokens::<T>(
			vault_id,
			consumed_issued_tokens,
			&Amount::zero(vault_id.collateral_currency()),
			&vault_id.account_id,
		)?;

		Pallet::<T>::release_replace_collateral(vault_id, consumed_issued_tokens)?;
		Ok(())
	}
}
// "Internal" functions, callable by code.
#[cfg_attr(test, mockable)]
impl<T: Config> Pallet<T> {
	pub fn _rate_limit_update(
		limit_volume_amount: Option<BalanceOf<T>>,
		limit_volume_currency_id: T::CurrencyId,
		interval_length: T::BlockNumber,
	) {
		<LimitVolumeAmount<T>>::set(limit_volume_amount);
		<LimitVolumeCurrencyId<T>>::set(limit_volume_currency_id);
		<IntervalLength<T>>::set(interval_length);
	}

	fn _request_redeem(
		redeemer: T::AccountId,
		amount_wrapped: BalanceOf<T>,
		stellar_address: StellarPublicKeyRaw,
		vault_id: DefaultVaultId<T>,
	) -> Result<H256, DispatchError> {
		let amount_wrapped = Amount::new(amount_wrapped, vault_id.wrapped_currency());

		// We ensure that the amount requested is compatible with the target chain (ie. it has a
		// specific amount of trailing zeros)
		amount_wrapped.ensure_is_compatible_with_target_chain()?;

		Self::check_volume(amount_wrapped.clone())?;

		ext::security::ensure_parachain_status_running::<T>()?;

		let redeemer_balance =
			ext::treasury::get_balance::<T>(&redeemer, vault_id.wrapped_currency());
		ensure!(amount_wrapped.le(&redeemer_balance)?, Error::<T>::AmountExceedsUserBalance);

		// todo: currently allowed to redeem from one currency to the other for free - decide if
		// this is desirable
		let fee_wrapped = if redeemer == vault_id.account_id {
			Amount::zero(vault_id.wrapped_currency())
		} else {
			ext::fee::get_redeem_fee::<T>(&amount_wrapped)?
		};
		let inclusion_fee = Self::get_current_inclusion_fee(vault_id.wrapped_currency())?;

		// We round the fee so that the amount of tokens that will be transferred on Stellar
		// are compatible without loss of precision.
		let fee_wrapped = fee_wrapped.round_to_target_chain()?;
		let vault_to_be_burned_tokens = amount_wrapped.checked_sub(&fee_wrapped)?;

		// this can overflow for small requested values. As such return
		// AmountBelowMinimumTransferAmount when this happens
		let user_to_be_received_stellar_asset = vault_to_be_burned_tokens
			.checked_sub(&inclusion_fee)
			.map_err(|_| Error::<T>::AmountBelowMinimumTransferAmount)?;

		ext::vault_registry::ensure_not_banned::<T>(&vault_id)?;

		// only allow requests of amount above above the minimum
		ensure!(
			// this is the amount the vault will send (minus fee)
			user_to_be_received_stellar_asset
				.ge(&Self::get_minimum_transfer_amount(vault_id.wrapped_currency()))?,
			Error::<T>::AmountBelowMinimumTransferAmount
		);

		// vault will get rid of the xlm + xlm_inclusion_fee
		ext::vault_registry::try_increase_to_be_redeemed_tokens::<T>(
			&vault_id,
			&vault_to_be_burned_tokens,
		)?;

		// lock full amount (inc. fee)
		amount_wrapped.lock_on(&redeemer)?;
		let redeem_id = ext::security::get_secure_id::<T>();

		let below_premium_redeem =
			ext::vault_registry::is_vault_below_premium_threshold::<T>(&vault_id)?;
		let currency_id = vault_id.collateral_currency();

		let premium_collateral = if below_premium_redeem {
			let redeem_amount_wrapped_in_collateral =
				user_to_be_received_stellar_asset.convert_to(currency_id)?;
			ext::fee::get_premium_redeem_fee::<T>(&redeem_amount_wrapped_in_collateral)?
		} else {
			Amount::zero(currency_id)
		};

		Self::release_replace_collateral(&vault_id, &vault_to_be_burned_tokens)?;

		Self::insert_redeem_request(
			&redeem_id,
			&RedeemRequest {
				vault: vault_id.clone(),
				opentime: ext::security::active_block_number::<T>(),
				fee: fee_wrapped.amount(),
				transfer_fee: inclusion_fee.amount(),
				amount: user_to_be_received_stellar_asset.amount(),
				asset: user_to_be_received_stellar_asset.currency(),
				premium: premium_collateral.amount(),
				period: Self::redeem_period(),
				redeemer: redeemer.clone(),
				stellar_address,
				status: RedeemRequestStatus::Pending,
			},
		);

		Self::deposit_event(Event::<T>::RequestRedeem {
			redeem_id,
			redeemer,
			amount: user_to_be_received_stellar_asset.amount(),
			asset: user_to_be_received_stellar_asset.currency(),
			fee: fee_wrapped.amount(),
			premium: premium_collateral.amount(),
			vault_id,
			stellar_address,
			transfer_fee: inclusion_fee.amount(),
		});

		Ok(redeem_id)
	}

	fn _liquidation_redeem(
		redeemer: T::AccountId,
		currencies: DefaultVaultCurrencyPair<T>,
		amount_wrapped: BalanceOf<T>,
	) -> Result<(), DispatchError> {
		let amount_wrapped = Amount::new(amount_wrapped, currencies.wrapped);

		let redeemer_balance = ext::treasury::get_balance::<T>(&redeemer, currencies.wrapped);
		ensure!(amount_wrapped.le(&redeemer_balance)?, Error::<T>::AmountExceedsUserBalance);

		amount_wrapped.lock_on(&redeemer)?;
		amount_wrapped.burn_from(&redeemer)?;
		ext::vault_registry::redeem_tokens_liquidation::<T>(
			currencies.collateral,
			&redeemer,
			&amount_wrapped,
		)?;

		// vault-registry emits `RedeemTokensLiquidation` with collateral amount
		Self::deposit_event(Event::<T>::LiquidationRedeem {
			redeemer,
			amount: amount_wrapped.amount(),
			asset: currencies.wrapped,
		});

		Ok(())
	}

	fn _execute_redeem(
		redeem_id: H256,
		transaction_envelope_xdr_encoded: Vec<u8>,
		externalized_envelopes_encoded: Vec<u8>,
		transaction_set_encoded: Vec<u8>,
	) -> Result<(), DispatchError> {
		let redeem = Self::get_open_redeem_request_from_id(&redeem_id)?;

		let transaction_envelope = ext::stellar_relay::construct_from_raw_encoded_xdr::<
			T,
			TransactionEnvelope,
		>(&transaction_envelope_xdr_encoded)?;

		let envelopes = ext::stellar_relay::construct_from_raw_encoded_xdr::<
			T,
			UnlimitedVarArray<ScpEnvelope>,
		>(&externalized_envelopes_encoded)?;

		let transaction_set = ext::stellar_relay::construct_from_raw_encoded_xdr::<
			T,
			TransactionSet,
		>(&transaction_set_encoded)?;

		// Check that the transaction includes the expected memo to mitigate replay attacks
		ext::stellar_relay::ensure_transaction_memo_matches_hash::<T>(
			&transaction_envelope,
			&redeem_id,
		)?;

		// Verify that the transaction is valid

		ext::stellar_relay::validate_stellar_transaction::<T>(
			&transaction_envelope,
			&envelopes,
			&transaction_set,
		)
		.map_err(|e| {
			log::error!(
				"failed to validate transaction of redeem id: {} with transaction envelope: {transaction_envelope:?}",
				hex::encode(redeem_id.as_bytes())
			);

			e
		})?;

		let paid_amount: Amount<T> = ext::currency::get_amount_from_transaction_envelope::<T>(
			&transaction_envelope,
			redeem.stellar_address,
			redeem.asset,
		)?;

		// Check that the transaction contains a payment with at least the expected amount
		ensure!(paid_amount.ge(&redeem.amount())?, Error::<T>::InvalidPaymentAmount);

		// burn amount (without parachain fee, but including transfer fee)
		let burn_amount = redeem.amount().checked_add(&redeem.transfer_fee())?;
		burn_amount.burn_from(&redeem.redeemer)?;
		// increase volume according to volume limits
		Self::increase_interval_volume(burn_amount.clone())?;

		// send fees to pool
		let fee = redeem.fee();
		fee.unlock_on(&redeem.redeemer)?;
		fee.transfer(&redeem.redeemer, &ext::fee::fee_pool_account_id::<T>())?;
		ext::fee::distribute_rewards::<T>(&fee)?;

		// next line fails
		ext::vault_registry::redeem_tokens::<T>(
			&redeem.vault,
			&burn_amount,
			&redeem.premium(),
			&redeem.redeemer,
		)?;

		Self::set_redeem_status(redeem_id, RedeemRequestStatus::Completed);
		Self::deposit_event(Event::<T>::ExecuteRedeem {
			redeem_id,
			redeemer: redeem.redeemer,
			vault_id: redeem.vault,
			amount: redeem.amount,
			asset: redeem.asset,
			fee: redeem.fee,
			transfer_fee: redeem.transfer_fee,
		});
		Ok(())
	}

	fn _cancel_redeem(redeemer: T::AccountId, redeem_id: H256, reimburse: bool) -> DispatchResult {
		ext::security::ensure_parachain_status_running::<T>()?;

		let redeem = Self::get_open_redeem_request_from_id(&redeem_id)?;
		ensure!(redeemer == redeem.redeemer, Error::<T>::UnauthorizedRedeemer);

		// only cancellable after the request has expired
		ensure!(
			ext::security::parachain_block_expired::<T>(
				redeem.opentime,
				Self::redeem_period().max(redeem.period)
			)?,
			Error::<T>::TimeNotExpired
		);

		let vault = ext::vault_registry::get_vault_from_id::<T>(&redeem.vault)?;
		let vault_to_be_redeemed_tokens =
			Amount::new(vault.to_be_redeemed_tokens, redeem.vault.wrapped_currency());
		let vault_id = redeem.vault.clone();

		let vault_to_be_burned_tokens = redeem.amount().checked_add(&redeem.transfer_fee())?;

		let amount_wrapped_in_collateral =
			vault_to_be_burned_tokens.convert_to(vault_id.collateral_currency())?;

		// now update the collateral; the logic is different for liquidated vaults.
		let slashed_amount = if vault.is_liquidated() {
			let confiscated_collateral = ext::vault_registry::calculate_collateral::<T>(
				&ext::vault_registry::get_liquidated_collateral::<T>(&redeem.vault)?,
				&vault_to_be_burned_tokens,
				&vault_to_be_redeemed_tokens, /* note: this is the value read prior to making
				                               * changes */
			)?;

			let slashing_destination = if reimburse {
				CurrencySource::FreeBalance(redeemer.clone())
			} else {
				CurrencySource::LiquidationVault(vault_id.currencies.clone())
			};
			ext::vault_registry::decrease_liquidated_collateral::<T>(
				&vault_id,
				&confiscated_collateral,
			)?;
			ext::vault_registry::transfer_funds::<T>(
				CurrencySource::LiquidatedCollateral(vault_id.clone()),
				slashing_destination,
				&confiscated_collateral,
			)?;

			confiscated_collateral
		} else {
			// not liquidated

			// calculate the punishment fee (e.g. 10%)
			let punishment_fee_in_collateral =
				ext::fee::get_punishment_fee::<T>(&amount_wrapped_in_collateral)?;

			let amount_to_slash = if reimburse {
				// 100% + punishment fee on reimburse
				amount_wrapped_in_collateral.checked_add(&punishment_fee_in_collateral)?
			} else {
				punishment_fee_in_collateral.clone()
			};

			let transferred_amount = ext::vault_registry::transfer_funds_saturated::<T>(
				CurrencySource::Collateral(vault_id.clone()),
				CurrencySource::FreeBalance(redeemer.clone()),
				&amount_to_slash,
			)?;

			// Subtract the punishment fee to avoid overpaying the vault when reclaiming tokens
			let retrievable_amount_in_collateral =
				transferred_amount.saturating_sub(&punishment_fee_in_collateral)?;
			let retrievable_amount_in_wrapped =
				retrievable_amount_in_collateral.convert_to(vault_id.wrapped_currency())?;

			CancelledRedeemAmount::<T>::insert(
				redeem_id,
				(retrievable_amount_in_wrapped.amount(), retrievable_amount_in_wrapped.currency()),
			);

			let _ = ext::vault_registry::ban_vault::<T>(&vault_id);

			amount_to_slash
		};

		// first update the issued tokens; this logic is the same regardless of whether or not the
		// vault is liquidated
		let new_status = if reimburse {
			// Transfer the transaction fee to the pool. Even though the redeem was not
			// successful, the user receives a premium in collateral, so it's OK to take the fee.
			let fee = redeem.fee();
			fee.unlock_on(&redeem.redeemer)?;
			fee.transfer(&redeem.redeemer, &ext::fee::fee_pool_account_id::<T>())?;
			ext::fee::distribute_rewards::<T>(&fee)?;

			if ext::vault_registry::is_vault_below_secure_threshold::<T>(&redeem.vault)? {
				// vault can not afford to back the tokens that it would receive, so we burn it
				vault_to_be_burned_tokens.burn_from(&redeemer)?;
				ext::vault_registry::decrease_tokens::<T>(
					&redeem.vault,
					&redeem.redeemer,
					&vault_to_be_burned_tokens,
				)?;
				Self::set_redeem_status(redeem_id, RedeemRequestStatus::Reimbursed(false))
			} else {
				// Transfer the rest of the user's issued tokens (i.e. excluding fee) to the vault
				vault_to_be_burned_tokens.unlock_on(&redeem.redeemer)?;
				vault_to_be_burned_tokens.transfer(&redeem.redeemer, &redeem.vault.account_id)?;
				ext::vault_registry::decrease_to_be_redeemed_tokens::<T>(
					&vault_id,
					&vault_to_be_burned_tokens,
				)?;
				Self::set_redeem_status(redeem_id, RedeemRequestStatus::Reimbursed(true))
			}
		} else {
			// unlock user's issued tokens, including fee
			let total_wrapped: Amount<T> = redeem
				.amount()
				.checked_add(&redeem.fee())?
				.checked_add(&redeem.transfer_fee())?;
			total_wrapped.unlock_on(&redeemer)?;
			ext::vault_registry::decrease_to_be_redeemed_tokens::<T>(
				&vault_id,
				&vault_to_be_burned_tokens,
			)?;
			Self::set_redeem_status(redeem_id, RedeemRequestStatus::Retried)
		};

		Self::deposit_event(Event::<T>::CancelRedeem {
			redeem_id,
			redeemer,
			vault_id: redeem.vault,
			slashed_amount: slashed_amount.amount(),
			status: new_status,
		});

		Ok(())
	}

	fn _mint_tokens_for_reimbursed_redeem(
		vault_id: DefaultVaultId<T>,
		redeem_id: H256,
	) -> DispatchResult {
		ext::security::ensure_parachain_status_running::<T>()?;

		let redeem =
			RedeemRequests::<T>::try_get(&redeem_id).or(Err(Error::<T>::RedeemIdNotFound))?;
		ensure!(
			matches!(redeem.status, RedeemRequestStatus::Reimbursed(false)),
			Error::<T>::RedeemCancelled
		);

		ensure!(redeem.vault == vault_id, Error::<T>::UnauthorizedVault);

		let reimbursed_amount = CancelledRedeemAmount::<T>::take(redeem_id)
			.map(|(amount, currency_id)| Amount::new(amount, currency_id))
			.unwrap_or(redeem.amount().checked_add(&redeem.transfer_fee())?);

		ext::vault_registry::try_increase_to_be_issued_tokens::<T>(&vault_id, &reimbursed_amount)?;
		ext::vault_registry::issue_tokens::<T>(&vault_id, &reimbursed_amount)?;
		reimbursed_amount.mint_to(&vault_id.account_id)?;

		Self::set_redeem_status(redeem_id, RedeemRequestStatus::Reimbursed(true));

		Self::deposit_event(Event::<T>::MintTokensForReimbursedRedeem {
			redeem_id,
			vault_id: redeem.vault,
			amount: reimbursed_amount.amount(),
		});

		Ok(())
	}

	fn release_replace_collateral(
		vault_id: &DefaultVaultId<T>,
		burned_tokens: &Amount<T>,
	) -> DispatchResult {
		// decrease to-be-replaced tokens - when the vault requests tokens to be replaced, it
		// want to get rid of tokens, and it does not matter whether this is through a redeem,
		// or a replace. As such, we decrease the to-be-replaced tokens here. This call will
		// never fail due to insufficient to-be-replaced tokens
		let (_, griefing_collateral) =
			ext::vault_registry::decrease_to_be_replaced_tokens::<T>(vault_id, burned_tokens)?;
		// release the griefing collateral that is locked for the replace request
		if !griefing_collateral.is_zero() {
			ext::vault_registry::transfer_funds(
				CurrencySource::AvailableReplaceCollateral(vault_id.clone()),
				CurrencySource::FreeBalance(vault_id.account_id.clone()),
				&griefing_collateral,
			)?;
		}
		Ok(())
	}

	/// Insert a new redeem request into state.
	///
	/// # Arguments
	///
	/// * `key` - 256-bit identifier of the redeem request
	/// * `value` - the redeem request
	fn insert_redeem_request(key: &H256, value: &DefaultRedeemRequest<T>) {
		<RedeemRequests<T>>::insert(key, value)
	}

	#[cfg(any(test, feature = "runtime-benchmarks"))]
	fn insert_cancelled_redeem_amount(redeem_id: H256, amount: Amount<T>) {
		let bal = amount.amount();
		let curr_id = amount.currency();
		<CancelledRedeemAmount<T>>::insert(redeem_id, (bal, curr_id));
	}

	fn set_redeem_status(id: H256, status: RedeemRequestStatus) -> RedeemRequestStatus {
		<RedeemRequests<T>>::mutate_exists(id, |request| {
			*request = request
				.clone()
				.map(|request| DefaultRedeemRequest::<T> { status: status.clone(), ..request });
		});

		status
	}

	/// Get the current inclusion fee based on the fee stats reported by the oracle
	pub fn get_current_inclusion_fee(
		wrapped_currency: CurrencyId<T>,
	) -> Result<Amount<T>, DispatchError> {
		// We don't charge a inclusion fee for now because fees on Stellar are so cheap
		let fee: BalanceOf<T> = 0i64.try_into().map_err(|_| Error::<T>::TryIntoIntError)?;
		let amount: Amount<T> = Amount::new(fee, wrapped_currency);
		Ok(amount)
	}

	pub fn get_minimum_transfer_amount(currency_id: CurrencyId<T>) -> Amount<T> {
		Amount::new(<RedeemMinimumTransferAmount<T>>::get(), currency_id)
	}

	/// Fetch all redeem requests for the specified account.
	///
	/// # Arguments
	///
	/// * `account_id` - user account id
	pub fn get_redeem_requests_for_account(account_id: T::AccountId) -> Vec<H256> {
		<RedeemRequests<T>>::iter()
			.filter(|(_, request)| request.redeemer == account_id)
			.map(|(key, _)| key)
			.collect::<Vec<_>>()
	}

	/// Fetch all redeem requests for the specified vault.
	///
	/// # Arguments
	///
	/// * `vault_id` - vault account id
	pub fn get_redeem_requests_for_vault(vault_id: T::AccountId) -> Vec<H256> {
		<RedeemRequests<T>>::iter()
			.filter(|(_, request)| request.vault.account_id == vault_id)
			.map(|(key, _)| key)
			.collect::<Vec<_>>()
	}

	/// Fetch a pre-existing redeem request or throw. Completed or cancelled
	/// requests are not returned.
	///
	/// # Arguments
	///
	/// * `redeem_id` - 256-bit identifier of the redeem request
	pub fn get_open_redeem_request_from_id(
		redeem_id: &H256,
	) -> Result<DefaultRedeemRequest<T>, DispatchError> {
		let request =
			RedeemRequests::<T>::try_get(redeem_id).or(Err(Error::<T>::RedeemIdNotFound))?;

		// NOTE: temporary workaround until we delete
		match request.status {
			RedeemRequestStatus::Pending => Ok(request),
			RedeemRequestStatus::Completed => Err(Error::<T>::RedeemCompleted.into()),
			RedeemRequestStatus::Reimbursed(_) | RedeemRequestStatus::Retried =>
				Err(Error::<T>::RedeemCancelled.into()),
		}
	}

	/// Fetch a pre-existing open or completed redeem request or throw.
	/// Cancelled requests are not returned.
	///
	/// # Arguments
	///
	/// * `redeem_id` - 256-bit identifier of the redeem request
	pub fn get_open_or_completed_redeem_request_from_id(
		redeem_id: &H256,
	) -> Result<DefaultRedeemRequest<T>, DispatchError> {
		let request =
			RedeemRequests::<T>::try_get(redeem_id).or(Err(Error::<T>::RedeemIdNotFound))?;

		ensure!(
			matches!(request.status, RedeemRequestStatus::Pending | RedeemRequestStatus::Completed),
			Error::<T>::RedeemCancelled
		);
		Ok(request)
	}

	fn increase_interval_volume(issue_amount: Amount<T>) -> Result<(), DispatchError> {
		if let Some(_limit_volume) = LimitVolumeAmount::<T>::get() {
			let issue_volume = Self::convert_into_limit_currency_id_amount(issue_amount)?;
			let current_volume = CurrentVolumeAmount::<T>::get();
			let new_volume = current_volume.saturating_add(issue_volume.amount());
			CurrentVolumeAmount::<T>::put(new_volume);
		}
		Ok(())
	}

	fn convert_into_limit_currency_id_amount(
		issue_amount: Amount<T>,
	) -> Result<Amount<T>, DispatchError> {
		let issue_volume =
			oracle::Pallet::<T>::convert(&issue_amount, LimitVolumeCurrencyId::<T>::get())
				.map_err(|_| DispatchError::Other("Missing Exchange Rate"))?;
		Ok(issue_volume)
	}

	fn check_volume(amount_requested: Amount<T>) -> Result<(), DispatchError> {
		let limit_volume: Option<BalanceOf<T>> = LimitVolumeAmount::<T>::get();
		if let Some(limit_volume) = limit_volume {
			let current_block: T::BlockNumber = <frame_system::Pallet<T>>::block_number();
			let interval_length: T::BlockNumber = IntervalLength::<T>::get();

			let current_index = current_block.checked_div(&interval_length);
			let mut current_volume = BalanceOf::<T>::zero();
			if current_index != Some(LastIntervalIndex::<T>::get()) {
				LastIntervalIndex::<T>::put(current_index.unwrap_or_default());
				CurrentVolumeAmount::<T>::put(current_volume);
			} else {
				current_volume = CurrentVolumeAmount::<T>::get();
			}
			let new_issue_request_amount =
				Self::convert_into_limit_currency_id_amount(amount_requested)?;
			ensure!(
				new_issue_request_amount.amount().saturating_add(current_volume) <= limit_volume,
				Error::<T>::ExceedLimitVolumeForIssueRequest
			);
		}
		Ok(())
	}
}
