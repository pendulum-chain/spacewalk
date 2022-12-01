//! # Currency Wrappers

#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{EncodeLike, FullCodec};
use frame_support::{dispatch::DispatchResult, traits::Get};
use orml_traits::{MultiCurrency, MultiReservableCurrency};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedDiv, StaticLookup},
	FixedPointNumber, FixedPointOperand,
};
use sp_std::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
	marker::PhantomData,
	vec::Vec,
};

pub use amount::Amount;
pub use pallet::*;
use primitives::{
	stellar::{
		types::{OperationBody, Uint256},
		ClaimPredicate, Claimant, MuxedAccount, Operation, PublicKey, TransactionEnvelope,
	},
	TruncateFixedPointToInt,
};
use types::*;
pub use types::{CurrencyConversion, CurrencyId};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod amount;

mod types;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use sp_runtime::traits::StaticLookup;

	use primitives::stellar::Asset;

	use super::*;

	/// ## Configuration
	/// The pallet's configuration trait.
	#[pallet::config]
	pub trait Config:
		frame_system::Config + orml_tokens::Config<Balance = BalanceOf<Self>>
	{
		type UnsignedFixedPoint: FixedPointNumber<Inner = BalanceOf<Self>>
			+ TruncateFixedPointToInt
			+ Encode
			+ EncodeLike
			+ Decode
			+ MaybeSerializeDeserialize
			+ TypeInfo;

		type SignedInner: Debug
			+ CheckedDiv
			+ TryFrom<BalanceOf<Self>>
			+ TryInto<BalanceOf<Self>>
			+ MaybeSerializeDeserialize;

		type SignedFixedPoint: FixedPointNumber<Inner = SignedInner<Self>>
			+ TruncateFixedPointToInt
			+ Encode
			+ EncodeLike
			+ Decode
			+ MaybeSerializeDeserialize;

		type Balance: AtLeast32BitUnsigned
			+ FixedPointOperand
			+ MaybeSerializeDeserialize
			+ FullCodec
			+ Copy
			+ Default
			+ Debug;

		/// Native currency e.g. INTR/KINT
		#[pallet::constant]
		type GetNativeCurrencyId: Get<CurrencyId<Self>>;

		/// Relay chain currency e.g. DOT/KSM
		#[pallet::constant]
		type GetRelayChainCurrencyId: Get<CurrencyId<Self>>;

		type AssetConversion: StaticLookup<Source = CurrencyId<Self>, Target = Asset>;
		type BalanceConversion: StaticLookup<Source = BalanceOf<Self>, Target = i64>;
		type CurrencyConversion: types::CurrencyConversion<Amount<Self>, CurrencyId<Self>>;
	}

	#[pallet::error]
	pub enum Error<T> {
		AssetConversionError,
		BalanceConversionError,
		TryIntoIntError,
		InvalidCurrency,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);
}

impl<T: Config> Pallet<T> {
	/// Accumulate the amounts of the specified currency that happened in the operations of a
	/// Stellar transaction
	pub fn get_amount_from_transaction_envelope(
		transaction_envelope: &TransactionEnvelope,
		recipient_stellar_address: Uint256,
		currency: CurrencyId<T>,
	) -> Result<Amount<T>, Error<T>> {
		let asset = T::AssetConversion::lookup(currency)
			.map_err(|_| Error::<T>::AssetConversionError.into())?;
		let recipient_account_muxed = MuxedAccount::KeyTypeEd25519(recipient_stellar_address);
		let recipient_account_pk = PublicKey::PublicKeyTypeEd25519(recipient_stellar_address);

		let tx_operations: Vec<Operation> = match transaction_envelope {
			TransactionEnvelope::EnvelopeTypeTxV0(env) => env.tx.operations.get_vec().clone(),
			TransactionEnvelope::EnvelopeTypeTx(env) => env.tx.operations.get_vec().clone(),
			TransactionEnvelope::EnvelopeTypeTxFeeBump(_) => Vec::new(),
			TransactionEnvelope::Default(_) => Vec::new(),
		};

		let mut transferred_amount: i64 = 0;
		for x in tx_operations {
			match x.body {
				OperationBody::Payment(payment) => {
					if payment.destination.eq(&recipient_account_muxed) && payment.asset == asset {
						transferred_amount = transferred_amount.saturating_add(payment.amount);
					}
				},
				OperationBody::CreateClaimableBalance(payment) => {
					// for security reasons, we only count operations that have the
					// recipient as a single claimer and unconditional claim predicate
					if payment.claimants.len() == 1 {
						let Claimant::ClaimantTypeV0(claimant) =
							payment.claimants.get_vec()[0].clone();

						if claimant.destination.eq(&recipient_account_pk) &&
							payment.asset == asset && claimant.predicate ==
							ClaimPredicate::ClaimPredicateUnconditional
						{
							transferred_amount = transferred_amount.saturating_add(payment.amount);
						}
					}
				},
				_ => {
					// ignore other operations
				},
			}
		}

		// `transferred_amount` is in stroops, so we need to convert it
		let balance = T::BalanceConversion::unlookup(transferred_amount);
		let amount: Amount<T> = Amount::new(balance, currency);
		Ok(amount)
	}
}

pub mod getters {
	use super::*;

	pub fn get_relay_chain_currency_id<T: Config>() -> CurrencyId<T> {
		<T as Config>::GetRelayChainCurrencyId::get()
	}

	pub fn get_native_currency_id<T: Config>() -> CurrencyId<T> {
		<T as Config>::GetNativeCurrencyId::get()
	}

	#[cfg(feature = "testing-utils")]
	pub fn get_wrapped_currency_id<T: Config>() -> CurrencyId<T> {
		// Return some wrapped currency id for convenience in tests
		// Is it even possible to get the wrapped currency id from the primitives?
		let default_wrapped_currency: CurrencyId<T> = CurrencyId::<T>::AlphaNum4 {
			code: *b"USDC",
			issuer: [
				20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231,
				46, 199, 108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
			],
		};
	}
}

pub fn get_free_balance<T: Config>(
	currency_id: T::CurrencyId,
	account: &AccountIdOf<T>,
) -> Amount<T> {
	let amount = <orml_tokens::Pallet<T>>::free_balance(currency_id, account);
	Amount::new(amount, currency_id)
}

pub fn get_reserved_balance<T: Config>(
	currency_id: T::CurrencyId,
	account: &AccountIdOf<T>,
) -> Amount<T> {
	let amount = <orml_tokens::Pallet<T>>::reserved_balance(currency_id, account);
	Amount::new(amount, currency_id)
}

pub trait OnSweep<AccountId, Balance> {
	fn on_sweep(who: &AccountId, amount: Balance) -> DispatchResult;
}

impl<AccountId, Balance> OnSweep<AccountId, Balance> for () {
	fn on_sweep(_: &AccountId, _: Balance) -> DispatchResult {
		Ok(())
	}
}

pub struct SweepFunds<T, GetAccountId>(PhantomData<(T, GetAccountId)>);

impl<T, GetAccountId> OnSweep<AccountIdOf<T>, Amount<T>> for SweepFunds<T, GetAccountId>
where
	T: Config,
	GetAccountId: Get<AccountIdOf<T>>,
{
	fn on_sweep(who: &AccountIdOf<T>, amount: Amount<T>) -> DispatchResult {
		// transfer the funds to treasury account
		amount.transfer(who, &GetAccountId::get())
	}
}
