#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

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
			NodeId, ScpEnvelope, ScpStatementExternalize, ScpStatementPledges, StellarValue,
			TransactionSet,
		},
		Hash, TransactionEnvelope, XdrCodec,
	};

	use crate::traits::{Organization, Validator};

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
		InvalidEnvelopeSignature,
		InvalidScpPledge,
		InvalidTransactionSet,
		InvalidTransactionXDR,
		InvalidQuorumSet,
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
		/// - `transaction_envelope`: The transaction envelope of the tx to be verified
		/// - `envelopes`: The set of SCP envelopes that were externalized on the Stellar network
		/// - `transaction_set`: The set of transactions that belong to the envelopes
		pub fn validate_stellar_transaction(
			transaction_envelope: TransactionEnvelope,
			envelopes: UnlimitedVarArray<ScpEnvelope>,
			transaction_set: TransactionSet,
			network: &Network,
		) -> DispatchResult {
			// Check if tx is included in the transaction set
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

				let signature_valid = verify_signature(envelope, &node_id, network);
				ensure!(signature_valid, Error::<T>::InvalidEnvelopeSignature);
			}

			// Check if transaction set matches tx_set_hash included in the ScpEnvelopes
			let expected_tx_set_hash = compute_non_generic_tx_set_content_hash(&transaction_set);

			for envelope in envelopes.get_vec() {
				match envelope.clone().statement.pledges {
					ScpStatementPledges::ScpStExternalize(externalized_statement) => {
						let tx_set_hash = Self::get_tx_set_hash(&externalized_statement)?;
						ensure!(
							tx_set_hash == expected_tx_set_hash,
							Error::<T>::TransactionSetHashMismatch
						);
					},
					_ => return Err(Error::<T>::InvalidScpPledge.into()),
				}
			}

			// ---- Check that externalized messages build valid quorum set ----
			let targeted_validators = validators
				.iter()
				.filter(|validator| {
					envelopes.get_vec().iter().any(|envelope| {
						envelope.statement.node_id.to_encoding() == validator.public_key.to_vec()
					})
				})
				.collect::<Vec<&Validator>>();

			let total_organizations = targeted_validators
				.iter()
				.map(|validator| validator.organization.clone())
				.collect::<Vec<Organization>>();

			// Build the distinct organizations
			let mut total_distinct_organizations = total_organizations.clone();
			total_distinct_organizations.sort_by(|a, b| a.name.cmp(&b.name));
			total_distinct_organizations.dedup_by(|a, b| a.name == b.name);

			// The organizations occurring related to the targeted validators
			let targeted_organizations = targeted_validators
				.iter()
				.map(|validator| validator.organization.clone())
				.collect::<Vec<Organization>>();

			// Build the distinct set of targeted organizations
			let mut targeted_distinct_organizations = targeted_organizations.clone();
			targeted_distinct_organizations.sort_by(|a, b| a.name.cmp(&b.name));
			targeted_distinct_organizations.dedup_by(|a, b| a.name == b.name);

			// Check that the distinct organizations occurring in the validator structs related to
			// the externalized messages are more than 2/3 of the total amount of organizations in
			// the tier 1 validator set.
			// Use multiplication to avoid floating point numbers.
			ensure!(
				targeted_distinct_organizations.len() * 3 > total_distinct_organizations.len() * 2,
				Error::<T>::InvalidQuorumSet
			);

			for organization in &targeted_distinct_organizations {
				let total = organization.total_org_nodes;
				let mut count = 0;

				// check if all organizations are higher than the threshold of 1/2
				for o in &targeted_organizations {
					if o.name == organization.name {
						count = count + 1;
					}
				}
				// More than half of each distinct organization has to be in the quorum set
				ensure!(count * 2 > total, Error::<T>::InvalidQuorumSet)
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
	}

	pub(crate) fn compute_non_generic_tx_set_content_hash(tx_set: &TransactionSet) -> [u8; 32] {
		let mut hasher = Sha256::new();
		hasher.update(tx_set.previous_ledger_hash);

		tx_set.txes.get_vec().iter().for_each(|envelope| {
			hasher.update(envelope.to_xdr());
		});

		hasher.finalize().as_slice().try_into().unwrap()
	}

	pub(crate) fn verify_signature(
		envelope: &ScpEnvelope,
		node_id: &NodeId,
		network: &Network,
	) -> bool {
		let mut vec: [u8; 64] = [0; 64];
		vec.copy_from_slice(envelope.signature.get_vec());
		let signature: &substrate_stellar_sdk::Signature = &vec;

		// Envelope_Type_SCP = 1, see https://github.dev/stellar/stellar-core/blob/d3b80614cb92f44b789ac79f3dee29ca09de6fdb/src/protocol-curr/xdr/Stellar-ledger-entries.x#L586
		let envelope_type_scp: Vec<u8> = [0, 0, 0, 1].to_vec(); // xdr representation
		let network = network.get_id();

		// Signature is created by signing concatenation of network id, SCP type and SCP statement
		let body: Vec<u8> =
			[network.to_vec(), envelope_type_scp, envelope.statement.to_xdr()].concat();

		node_id.verify_signature(body, signature)
	}
}
