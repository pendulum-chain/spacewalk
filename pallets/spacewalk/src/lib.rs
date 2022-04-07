#![cfg_attr(not(feature = "std"), no_std)]
#![feature(result_flattening)]

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod address_conv;
pub mod balance_conv;
pub mod currency;
pub mod currency_conv;
mod horizon;

use codec::{Decode, Encode};
use orml_traits::MultiCurrency;
pub use pallet::*;
use pallet_transaction_payment::Config as PaymentConfig;
use sp_core::crypto::KeyTypeId;
use sp_runtime::traits::{Convert, StaticLookup};
use sp_runtime::RuntimeDebug;
use sp_std::{convert::From, prelude::*, str};

use substrate_stellar_sdk as stellar;

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;

type BalanceOf<T> = <<T as Config>::Currency as orml_traits::MultiCurrency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

type CurrencyIdOf<T> =
    <<T as Config>::Currency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"abcd");

/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrapper.
/// We can utilize the supported crypto kinds (`ed25519`, `ed25519` and `ecdsa`) and augment
/// them with the pallet-specific identifier.
pub mod crypto {
    use super::KEY_TYPE;
    use sp_runtime::app_crypto::{app_crypto, ed25519};

    app_crypto!(ed25519, KEY_TYPE);
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct DepositPayload<Currency, AccountId, Public, Balance> {
    currency_id: Currency,
    amount: Balance,
    destination: AccountId,
    signed_by: Public,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::error::LookupError;
    use stellar::{
        types::{OperationBody, PaymentOp},
        XdrCodec,
    };

    #[pallet::config]
    pub trait Config: frame_system::Config + PaymentConfig + orml_tokens::Config {
        /// The overarching dispatch call type.
        type Call: From<Call<Self>>;
        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// The mechanics of the ORML tokens
        type Currency: MultiCurrency<<Self as frame_system::Config>::AccountId>;
        type AddressConversion: StaticLookup<
            Source = <Self as frame_system::Config>::AccountId,
            Target = substrate_stellar_sdk::PublicKey,
        >;
        type BalanceConversion: StaticLookup<Source = BalanceOf<Self>, Target = i64>;
        type StringCurrencyConversion: Convert<(Vec<u8>, Vec<u8>), Result<CurrencyIdOf<Self>, ()>>;

        /// Conversion between Stellar asset type and this pallet trait for Currency
        type CurrencyConversion: StaticLookup<
            Source = CurrencyIdOf<Self>,
            Target = substrate_stellar_sdk::Asset,
        >;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Event generated when a new deposit is made on a Stellar Account.
        Deposit(
            CurrencyIdOf<T>,
            <T as frame_system::Config>::AccountId,
            BalanceOf<T>,
        ),
        /// User initiated a redeem. [CurrencyIdOf<T>, T::AccountId, BalanceOf<T>]
		Redeem(CurrencyIdOf<T>, T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        // Error returned when making signed transactions in off-chain worker
        NoLocalAcctForSigning,

        // XDR encoding/decoding error
        XdrCodecError,

        // Failed to change a balance
        BalanceChangeError,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // TODO Benchmakr weights
        #[pallet::weight(10_000)]
        pub fn report_stellar_transaction(
            origin: OriginFor<T>,
            transaction_envelope_xdr: Vec<u8>,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            let tx_xdr = base64::decode(&transaction_envelope_xdr).unwrap();
            let tx_envelope =
                substrate_stellar_sdk::TransactionEnvelope::from_xdr(&tx_xdr).unwrap();

            if let substrate_stellar_sdk::TransactionEnvelope::EnvelopeTypeTx(env) = tx_envelope {
                Self::process_new_transaction(env.tx);
            }
            Ok(())
        }
 
        #[pallet::weight(100000)]
        pub fn redeem(
            origin: OriginFor<T>,
            asset_code: Vec<u8>,
            asset_issuer: Vec<u8>,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
			let currency_id = T::StringCurrencyConversion::convert((asset_code, asset_issuer))
                .map_err(|_| LookupError)?;
            let pendulum_account_id = ensure_signed(origin)?;
            //let stellar_address = T::AddressConversion::lookup(pendulum_account_id.clone())?;

            T::Currency::withdraw(currency_id.clone(), &pendulum_account_id, amount)
                .map_err(|_| <Error<T>>::BalanceChangeError)?;

            Self::deposit_event(Event::Redeem(currency_id, pendulum_account_id, amount));
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        fn process_new_transaction(transaction: stellar::types::Transaction) {
            // The destination of a mirrored Pendulum transaction, is always derived of the source
            // account that initiated the Stellar transaction.
            let destination = if let substrate_stellar_sdk::MuxedAccount::KeyTypeEd25519(key) =
                transaction.source_account
            {
                T::AddressConversion::unlookup(substrate_stellar_sdk::PublicKey::from_binary(key))
            } else {
                log::error!("❌  Source account format not supported.");
                return;
            };

            let payment_ops: Vec<&PaymentOp> = transaction
                .operations
                .get_vec()
                .into_iter()
                .filter_map(|op| match &op.body {
                    OperationBody::Payment(p) => Some(p),
                    _ => None,
                })
                .collect();
            for payment_op in payment_ops {
                let amount = T::BalanceConversion::unlookup(payment_op.amount);
                let currency = T::CurrencyConversion::unlookup(payment_op.asset.clone());

                match Self::send_payment_tx(currency, amount, destination.clone()) {
                    Err(_) => log::warn!("Sending the tx failed."),
                    Ok(_) => {
                        log::info!("✅ Deposit successfully Executed");
                        ()
                    }
                }
            }
        }

        fn send_payment_tx(
            currency_id: CurrencyIdOf<T>,
            amount: BalanceOf<T>,
            destination: <T as frame_system::Config>::AccountId,
        ) -> Result<(), Error<T>> {
            let result = T::Currency::deposit(currency_id, &destination, amount);
            log::info!("{:?}", result);

            Self::deposit_event(Event::Deposit(currency_id, destination, amount));
            Ok(())
        }
    }
}
