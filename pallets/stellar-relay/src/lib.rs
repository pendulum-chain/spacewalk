#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod traits;
mod types;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sha2::{Digest, Sha256};
	use substrate_stellar_sdk::{
		compound_types::UnlimitedVarArray,
		network::Network,
		types::{
			ScpEnvelope, ScpStatementExternalize, ScpStatementPledges, StellarValue, TransactionSet,
		},
		Hash, TransactionEnvelope, XdrCodec,
	};

	use crate::traits::Validator;

	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		// The maximum amount of validators stored on-chain
		#[pallet::constant]
		type ValidatorLimit: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		Base64DecodeError,
		BoundedVecCreationFailed,
		EnvelopeSignedByUnknownValidator,
		InvalidExternalizedMessages,
		InvalidScpPledge,
		InvalidTransactionSet,
		InvalidTransactionXDR,
		TransactionNotInTransactionSet,
		TransactionSetHashMismatch,
		TransactionSetHashCreationFailed,
		ValidatorLimitExceeded,
	}

	#[pallet::storage]
	#[pallet::getter(fn validators)]
	pub type Validators<T: Config> =
		StorageValue<_, BoundedVec<Validator, T::ValidatorLimit>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub validators: Vec<Validator>,
		phantom: PhantomData<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { validators: vec![], phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let vec = BoundedVec::<Validator, T::ValidatorLimit>::try_from(self.validators.clone());
			assert!(vec.is_ok());
			Validators::<T>::put(vec.unwrap());
		}
	}

	// Extrinsics
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// This extrinsic is used to update/replace the current set of validators.
		#[pallet::weight(10_000)]
		pub fn update_tier_1_validator_set(
			origin: OriginFor<T>,
			validators: Vec<Validator>,
		) -> DispatchResult {
			// Limit this call to root
			let _ = ensure_root(origin)?;

			// Ensure that the number of validators does not exceed the limit
			ensure!(
				validators.len() as u32 <= T::ValidatorLimit::get(),
				Error::<T>::ValidatorLimitExceeded
			);

			let vec = BoundedVec::<Validator, T::ValidatorLimit>::try_from(validators)
				.map_err(|_| Error::<T>::BoundedVecCreationFailed)?;
			Validators::<T>::put(vec);

			Ok(())
		}
	}

	// Helper functions
	impl<T: Config> Pallet<T> {
		/// This function is used to verify if a give transaction was executed on the Stellar
		/// network.
		/// Parameters:
		/// - `transaction_envelope_xdr_encoded`: The XDR of the base64-encoded transaction envelope
		///   to verify
		/// - `externalized_envelopes_encoded`: The XDR of the base64-encoded externalized
		///   ScpEnvelopes
		/// - `transaction_set_encoded`: The XDR of the base64-encoded transaction set to use for
		///   verification
		pub fn validate_stellar_transaction(
			transaction_envelope_xdr_encoded: Vec<u8>,
			externalized_envelopes_encoded: Vec<u8>,
			transaction_set_encoded: Vec<u8>,
		) -> DispatchResult {
			let tx_xdr = base64::decode(&transaction_envelope_xdr_encoded)
				.map_err(|_| Error::<T>::Base64DecodeError)?;
			let transaction_envelope = TransactionEnvelope::from_xdr(tx_xdr)
				.map_err(|_| Error::<T>::InvalidTransactionXDR)?;
			println!("transaction_envelope: {:?}", transaction_envelope);

			let envelopes_xdr = base64::decode(&externalized_envelopes_encoded)
				.map_err(|_| Error::<T>::Base64DecodeError)?;
			let envelopes = UnlimitedVarArray::<ScpEnvelope>::from_xdr(envelopes_xdr)
				.map_err(|_| Error::<T>::InvalidExternalizedMessages)?;
			println!("envelopes: {:?}", envelopes);

			let transaction_set_xdr = base64::decode(&transaction_set_encoded)
				.map_err(|_| Error::<T>::Base64DecodeError)?;
			let transaction_set = TransactionSet::from_xdr(transaction_set_xdr)
				.map_err(|_| Error::<T>::InvalidTransactionSet)?;

			println!("transaction_set: {:?}", transaction_set);

			// Check if tx is included in the transaction set
			// TODO - make network configurable
			let network = Network::new(b"Public Global Stellar Network ; September 2015");

			let tx_hash = transaction_envelope.get_hash(&network);
			let tx_included =
				transaction_set.txes.get_vec().iter().any(|tx| tx.get_hash(&network) == tx_hash);
			ensure!(tx_included, Error::<T>::TransactionNotInTransactionSet);

			// Check if all externalized ScpEnvelopes were signed by a tier 1 validator
			let validators = Validators::<T>::get();
			for envelope in envelopes.get_vec() {
				let node_id = envelope.statement.node_id.clone();
				let node_id_found = validators
					.iter()
					.any(|validator| validator.public_key.to_vec() == node_id.to_encoding());

				ensure!(node_id_found, Error::<T>::EnvelopeSignedByUnknownValidator);
				// TODO - Check if signature is valid
			}

			// Check if transaction set matches tx_set_hash included in the ScpEnvelopes
			let expected_tx_set_hash =
				Self::compute_non_generic_tx_set_content_hash(&transaction_set);

			println!("expected_tx_set_hash: {:?}", expected_tx_set_hash);

			for envelope in envelopes.get_vec() {
				match envelope.clone().statement.pledges {
					ScpStatementPledges::ScpStExternalize(externalized_statement) => {
						let tx_set_hash = Self::get_tx_set_hash(&externalized_statement)?;
						println!("tx_set_hash: {:?}", tx_set_hash);
						ensure!(
							tx_set_hash == expected_tx_set_hash,
							Error::<T>::TransactionSetHashMismatch
						);
					},
					_ => return Err(Error::<T>::InvalidScpPledge.into()),
				}
			}

			Ok(())
		}

		fn get_tx_set_hash(x: &ScpStatementExternalize) -> Result<Hash, DispatchError> {
			let scp_value = x.commit.value.get_vec();
			let tx_set_hash = StellarValue::from_xdr(scp_value)
				.map(|stellar_value| stellar_value.tx_set_hash)
				.map_err(|_| Error::<T>::TransactionSetHashCreationFailed)?;
			Ok(tx_set_hash)
		}

		fn compute_non_generic_tx_set_content_hash(tx_set: &TransactionSet) -> [u8; 32] {
			let mut hasher = Sha256::new();
			hasher.update(tx_set.previous_ledger_hash);

			tx_set.txes.get_vec().iter().for_each(|envlp| {
				hasher.update(envlp.to_xdr());
			});

			hasher.finalize().as_slice().try_into().unwrap()
		}
	}
}
