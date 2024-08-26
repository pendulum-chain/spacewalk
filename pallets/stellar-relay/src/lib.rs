#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

pub use default_weights::{SubstrateWeight, WeightInfo};
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

#[cfg(feature = "testing-utils")]
pub mod testing_utils;

pub mod traits;
pub mod types;

mod default_weights;
mod validation;

use primitives::{derive_shortened_request_id, get_text_memo_from_tx_env, TextMemo};

#[frame_support::pallet]
pub mod pallet {
	use codec::FullCodec;
	use frame_support::{pallet_prelude::*, sp_runtime, transactional};
	use frame_system::pallet_prelude::*;
	use primitives::stellar::{
		compound_types::UnlimitedVarArray,
		network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
		types::{NodeId, ScpEnvelope, StellarValue, Value},
		Hash, TransactionEnvelope, TransactionSetType, XdrCodec,
	};

	use sp_core::H256;
	use sp_std::{collections::btree_map::BTreeMap, fmt::Debug, vec, vec::Vec};

	use default_weights::WeightInfo;

	use crate::{
		traits::FieldLength,
		types::{OrganizationOf, ValidatorOf},
		validation::{
			check_for_valid_quorum_set, find_externalized_envelope, get_externalized_info,
			validate_envelopes, validators_and_orgs,
		},
	};

	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type OrganizationId: Clone
			+ Copy
			+ Debug
			+ Default
			+ Eq
			+ From<u32>
			+ FullCodec
			+ MaxEncodedLen
			+ MaybeSerializeDeserialize
			+ Ord
			+ PartialEq
			+ TypeInfo;

		// The maximum amount of organizations stored on-chain
		#[pallet::constant]
		type OrganizationLimit: Get<u32>;

		// The maximum amount of validators stored on-chain
		#[pallet::constant]
		type ValidatorLimit: Get<u32>;

		#[pallet::constant]
		type IsPublicNetwork: Get<bool>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		UpdateTier1ValidatorSet { new_validators_enactment_block_height: BlockNumberFor<T> },
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		Base64DecodeError,
		BoundedVecCreationFailed,
		DuplicateOrganizationId,
		DuplicateValidatorPublicKey,
		EmptyEnvelopeSet,
		EnvelopeSignedByUnknownValidator,
		EnvelopeSlotIndexMismatch,
		ExternalizedNHMismatch,
		ExternalizedValueMismatch,
		FailedToComputeNonGenericTxSetContentHash,
		InvalidEnvelopeSignature,
		InvalidQuorumSetNotEnoughOrganizations,
		InvalidQuorumSetNotEnoughValidators,
		InvalidScpPledge,
		InvalidTransactionSetPrefix,
		InvalidXDR,
		MissingExternalizedMessage,
		NoOrganizationsRegistered,
		NoValidatorsRegistered,
		OrganizationLimitExceeded,
		SlotIndexIsNone,
		TransactionMemoDoesNotMatch,
		TransactionNotInTransactionSet,
		TransactionSetHashCreationFailed,
		TransactionSetHashMismatch,
		ValidatorLimitExceeded,
	}

	#[pallet::storage]
	#[pallet::getter(fn organizations)]
	pub type Organizations<T: Config> =
		StorageValue<_, BoundedVec<OrganizationOf<T>, T::OrganizationLimit>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn validators)]
	pub type Validators<T: Config> =
		StorageValue<_, BoundedVec<ValidatorOf<T>, T::ValidatorLimit>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn old_organizations)]
	pub type OldOrganizations<T: Config> =
		StorageValue<_, BoundedVec<OrganizationOf<T>, T::OrganizationLimit>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn old_validators)]
	pub type OldValidators<T: Config> =
		StorageValue<_, BoundedVec<ValidatorOf<T>, T::ValidatorLimit>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn new_validators_enactment_block_height)]
	pub type NewValidatorsEnactmentBlockHeight<T: Config> =
		StorageValue<_, BlockNumberFor<T>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub old_validators: Vec<ValidatorOf<T>>,
		pub old_organizations: Vec<OrganizationOf<T>>,
		pub validators: Vec<ValidatorOf<T>>,
		pub organizations: Vec<OrganizationOf<T>>,
		pub enactment_block_height: BlockNumberFor<T>,
		pub phantom: PhantomData<T>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			// Create public network organizations
			let organization_sdf = OrganizationOf::<T> {
				name: create_bounded_vec("Stellar Development Foundation"),
				id: 0.into(),
			};
			let organization_satoshipay =
				OrganizationOf::<T> { name: create_bounded_vec("SatoshiPay"), id: 1.into() };
			let organization_whalestack =
				OrganizationOf::<T> { name: create_bounded_vec("Whalestack"), id: 2.into() };
			let organization_blockdaemon =
				OrganizationOf::<T> { name: create_bounded_vec("Blockdaemon"), id: 3.into() };
			let organization_lobstr =
				OrganizationOf::<T> { name: create_bounded_vec("LOBSTR"), id: 4.into() };
			let organization_public_node =
				OrganizationOf::<T> { name: create_bounded_vec("Public Node"), id: 5.into() };
			let organization_franklin_templeton = OrganizationOf::<T> {
				name: create_bounded_vec("Franklin Templeton"),
				id: 6.into(),
			};

			let validators: Vec<ValidatorOf<T>> = vec![
				// SDF validators
				ValidatorOf::<T> {
					name: create_bounded_vec("$sdf1"),
					public_key: create_bounded_vec(
						"GCGB2S2KGYARPVIA37HYZXVRM2YZUEXA6S33ZU5BUDC6THSB62LZSTYH",
					),
					organization_id: organization_sdf.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$sdf2"),
					public_key: create_bounded_vec(
						"GCM6QMP3DLRPTAZW2UZPCPX2LF3SXWXKPMP3GKFZBDSF3QZGV2G5QSTK",
					),
					organization_id: organization_sdf.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$sdf3"),
					public_key: create_bounded_vec(
						"GABMKJM6I25XI4K7U6XWMULOUQIQ27BCTMLS6BYYSOWKTBUXVRJSXHYQ",
					),
					organization_id: organization_sdf.id,
				},
				// Satoshipay validators
				ValidatorOf::<T> {
					name: create_bounded_vec("$satoshipay-us"),
					public_key: create_bounded_vec(
						"GAK6Z5UVGUVSEK6PEOCAYJISTT5EJBB34PN3NOLEQG2SUKXRVV2F6HZY",
					),
					organization_id: organization_satoshipay.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$satoshipay-de"),
					public_key: create_bounded_vec(
						"GC5SXLNAM3C4NMGK2PXK4R34B5GNZ47FYQ24ZIBFDFOCU6D4KBN4POAE",
					),
					organization_id: organization_satoshipay.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$satoshipay-sg"),
					public_key: create_bounded_vec(
						"GBJQUIXUO4XSNPAUT6ODLZUJRV2NPXYASKUBY4G5MYP3M47PCVI55MNT",
					),
					organization_id: organization_satoshipay.id,
				},
				// Coinqvest validators
				ValidatorOf::<T> {
					name: create_bounded_vec("$whalestack-germany"),
					public_key: create_bounded_vec(
						"GD6SZQV3WEJUH352NTVLKEV2JM2RH266VPEM7EH5QLLI7ZZAALMLNUVN",
					),
					organization_id: organization_whalestack.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$whalestack-finland"),
					public_key: create_bounded_vec(
						"GADLA6BJK6VK33EM2IDQM37L5KGVCY5MSHSHVJA4SCNGNUIEOTCR6J5T",
					),
					organization_id: organization_whalestack.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$whalestack-hongkong"),
					public_key: create_bounded_vec(
						"GAZ437J46SCFPZEDLVGDMKZPLFO77XJ4QVAURSJVRZK2T5S7XUFHXI2Z",
					),
					organization_id: organization_whalestack.id,
				},
				// Blockdaemon validators
				ValidatorOf::<T> {
					name: create_bounded_vec("$blockdaemon1"),
					public_key: create_bounded_vec(
						"GAAV2GCVFLNN522ORUYFV33E76VPC22E72S75AQ6MBR5V45Z5DWVPWEU",
					),
					organization_id: organization_blockdaemon.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$blockdaemon2"),
					public_key: create_bounded_vec(
						"GAVXB7SBJRYHSG6KSQHY74N7JAFRL4PFVZCNWW2ARI6ZEKNBJSMSKW7C",
					),
					organization_id: organization_blockdaemon.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$blockdaemon3"),
					public_key: create_bounded_vec(
						"GAYXZ4PZ7P6QOX7EBHPIZXNWY4KCOBYWJCA4WKWRKC7XIUS3UJPT6EZ4",
					),
					organization_id: organization_blockdaemon.id,
				},
				// LOBSTR validators
				ValidatorOf::<T> {
					name: create_bounded_vec("$lobstr1"),
					public_key: create_bounded_vec(
						"GCFONE23AB7Y6C5YZOMKUKGETPIAJA4QOYLS5VNS4JHBGKRZCPYHDLW7",
					),
					organization_id: organization_lobstr.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$lobstr2"),
					public_key: create_bounded_vec(
						"GCB2VSADESRV2DDTIVTFLBDI562K6KE3KMKILBHUHUWFXCUBHGQDI7VL",
					),
					organization_id: organization_lobstr.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$lobstr3"),
					public_key: create_bounded_vec(
						"GD5QWEVV4GZZTQP46BRXV5CUMMMLP4JTGFD7FWYJJWRL54CELY6JGQ63",
					),
					organization_id: organization_lobstr.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$lobstr4"),
					public_key: create_bounded_vec(
						"GA7TEPCBDQKI7JQLQ34ZURRMK44DVYCIGVXQQWNSWAEQR6KB4FMCBT7J",
					),
					organization_id: organization_lobstr.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$lobstr5"),
					public_key: create_bounded_vec(
						"GA5STBMV6QDXFDGD62MEHLLHZTPDI77U3PFOD2SELU5RJDHQWBR5NNK7",
					),
					organization_id: organization_lobstr.id,
				},
				// Public Node validators
				ValidatorOf::<T> {
					name: create_bounded_vec("$hercules"),
					public_key: create_bounded_vec(
						"GBLJNN3AVZZPG2FYAYTYQKECNWTQYYUUY2KVFN2OUKZKBULXIXBZ4FCT",
					),
					organization_id: organization_public_node.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$bootes"),
					public_key: create_bounded_vec(
						"GCVJ4Z6TI6Z2SOGENSPXDQ2U4RKH3CNQKYUHNSSPYFPNWTLGS6EBH7I2",
					),
					organization_id: organization_public_node.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$lyra"),
					public_key: create_bounded_vec(
						"GCIXVKNFPKWVMKJKVK2V4NK7D4TC6W3BUMXSIJ365QUAXWBRPPJXIR2Z",
					),
					organization_id: organization_public_node.id,
				},
				// Franklin Templeton validators
				ValidatorOf::<T> {
					name: create_bounded_vec("FTSCV1"),
					public_key: create_bounded_vec(
						"GARYGQ5F2IJEBCZJCBNPWNWVDOFK7IBOHLJKKSG2TMHDQKEEC6P4PE4V",
					),
					organization_id: organization_franklin_templeton.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("FTSCV2"),
					public_key: create_bounded_vec(
						"GCMSM2VFZGRPTZKPH5OABHGH4F3AVS6XTNJXDGCZ3MKCOSUBH3FL6DOB",
					),
					organization_id: organization_franklin_templeton.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("FTSCV3"),
					public_key: create_bounded_vec(
						"GA7DV63PBUUWNUFAF4GAZVXU2OZMYRATDLKTC7VTCG7AU4XUPN5VRX4A",
					),
					organization_id: organization_franklin_templeton.id,
				},
			];

			let organizations: Vec<OrganizationOf<T>> = vec![
				organization_satoshipay,
				organization_sdf,
				organization_franklin_templeton,
				organization_whalestack,
				organization_blockdaemon,
				organization_lobstr,
				organization_public_node,
			];

			// By default we initialize the old validators and organizations with an empty vec to
			// save space on chain
			let old_organizations = vec![];
			let old_validators = vec![];
			let enactment_block_height = BlockNumberFor::<T>::default();

			GenesisConfig {
				old_organizations,
				old_validators,
				validators,
				organizations,
				enactment_block_height,
				phantom: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			let old_validator_vec = BoundedVec::<ValidatorOf<T>, T::ValidatorLimit>::try_from(
				self.old_validators.clone(),
			);
			assert!(old_validator_vec.is_ok());
			OldValidators::<T>::put(old_validator_vec.unwrap());

			let validator_vec =
				BoundedVec::<ValidatorOf<T>, T::ValidatorLimit>::try_from(self.validators.clone());
			assert!(validator_vec.is_ok());
			Validators::<T>::put(validator_vec.unwrap());

			let old_organization_vec =
				BoundedVec::<OrganizationOf<T>, T::OrganizationLimit>::try_from(
					self.old_organizations.clone(),
				);
			assert!(old_organization_vec.is_ok());
			OldOrganizations::<T>::put(old_organization_vec.unwrap());

			let organization_vec = BoundedVec::<OrganizationOf<T>, T::OrganizationLimit>::try_from(
				self.organizations.clone(),
			);
			assert!(organization_vec.is_ok());
			Organizations::<T>::put(organization_vec.unwrap());

			NewValidatorsEnactmentBlockHeight::<T>::put(self.enactment_block_height);
		}
	}

	// Extrinsics
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// This extrinsic is used to update the current sets of validators and
		/// organizations. The current values of validators and organizations are moved to the
		/// OldValidators and OldOrganizations respectively, and the function arguments are stored
		/// as new/current values. The `enactment_block_height` parameter is used by the
		/// `validate_stellar_transaction` function to determine if it should use the 'old' or
		/// updated sets for validation. This makes a seamless transition between old and new
		/// validators possible.
		///
		/// It can only be called by the root origin.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::update_tier_1_validator_set())]
		#[transactional]
		pub fn update_tier_1_validator_set(
			origin: OriginFor<T>,
			validators: Vec<ValidatorOf<T>>,
			organizations: Vec<OrganizationOf<T>>,
			enactment_block_height: BlockNumberFor<T>,
		) -> DispatchResult {
			// Limit this call to root
			ensure_root(origin)?;

			Self::_update_tier_1_validator_set(validators, organizations, enactment_block_height)
		}
	}

	// Helper functions
	impl<T: Config> Pallet<T> {
		pub fn _update_tier_1_validator_set(
			validators: Vec<ValidatorOf<T>>,
			organizations: Vec<OrganizationOf<T>>,
			enactment_block_height: BlockNumberFor<T>,
		) -> DispatchResult {
			// Ensure that the number of validators does not exceed the limit
			ensure!(
				validators.len() as u32 <= T::ValidatorLimit::get(),
				Error::<T>::ValidatorLimitExceeded
			);
			// Ensure that the number of organizations does not exceed the limit
			ensure!(
				organizations.len() as u32 <= T::OrganizationLimit::get(),
				Error::<T>::OrganizationLimitExceeded
			);

			let mut organization_id_set = BTreeMap::<T::OrganizationId, u32>::new();
			for organization in organizations.iter() {
				organization_id_set
					.entry(organization.id)
					.and_modify(|e| {
						*e += 1;
					})
					.or_insert(1);
			}

			// If the length of the set does not match the length of the original vector we know
			// that there was a duplicate
			ensure!(
				organizations.len() == organization_id_set.len(),
				Error::<T>::DuplicateOrganizationId
			);

			let mut validators_public_key_set = BTreeMap::<BoundedVec<u8, FieldLength>, u32>::new();
			for validator in validators.iter() {
				validators_public_key_set
					.entry(validator.public_key.clone())
					.and_modify(|e| {
						*e += 1;
					})
					.or_insert(1);
			}

			// If the length of the set does not match the length of the original vector we know
			// that there was a duplicate
			ensure!(
				validators.len() == validators_public_key_set.len(),
				Error::<T>::DuplicateValidatorPublicKey
			);

			let current_validators = Validators::<T>::get();
			let current_organizations = Organizations::<T>::get();

			let new_validator_vec =
				BoundedVec::<ValidatorOf<T>, T::ValidatorLimit>::try_from(validators)
					.map_err(|_| Error::<T>::BoundedVecCreationFailed)?;

			let new_organization_vec =
				BoundedVec::<OrganizationOf<T>, T::OrganizationLimit>::try_from(organizations)
					.map_err(|_| Error::<T>::BoundedVecCreationFailed)?;

			NewValidatorsEnactmentBlockHeight::<T>::put(enactment_block_height);

			// update only when new organization or validators not equal to old organization or
			// validators
			if new_organization_vec != current_organizations ||
				new_validator_vec != current_validators
			{
				OldValidators::<T>::put(current_validators);
				OldOrganizations::<T>::put(current_organizations);
				Validators::<T>::put(new_validator_vec);
				Organizations::<T>::put(new_organization_vec);
			}

			Self::deposit_event(Event::<T>::UpdateTier1ValidatorSet {
				new_validators_enactment_block_height: enactment_block_height,
			});

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
		/// - `transaction_set`: The set of transactions that belong to the envelopes.
		pub fn validate_stellar_transaction(
			transaction_envelope: &TransactionEnvelope,
			envelopes: &UnlimitedVarArray<ScpEnvelope>,
			transaction_set: &TransactionSetType,
		) -> Result<(), Error<T>> {
			// Make sure that the envelope set is not empty
			ensure!(!envelopes.len() > 0, Error::<T>::EmptyEnvelopeSet);

			let network: &Network =
				if T::IsPublicNetwork::get() { &PUBLIC_NETWORK } else { &TEST_NETWORK };

			let tx_hash = transaction_envelope.get_hash(network);

			// Check if tx is included in the transaction set
			let tx_included = transaction_set
				.txes()
				.get_vec()
				.iter()
				.any(|tx| tx.get_hash(network) == tx_hash);
			ensure!(tx_included, Error::<T>::TransactionNotInTransactionSet);

			let (validators, organizations) = validators_and_orgs()?;

			let externalized_envelope = find_externalized_envelope(envelopes)?;

			// We store the externalized value in a variable so that we can check if it's the same
			// for all envelopes. We don't distinguish between externalized and confirmed values as
			// it should be the same value regardless.
			let (externalized_value, externalized_n_h) =
				get_externalized_info::<T>(externalized_envelope)
					.map_err(|_| Error::<T>::MissingExternalizedMessage)?;

			// Check if transaction set matches tx_set_hash included in the ScpEnvelopes
			let expected_tx_set_hash = transaction_set
				.get_tx_set_hash()
				.map_err(|_| Error::<T>::FailedToComputeNonGenericTxSetContentHash)?;

			validate_envelopes(
				envelopes,
				&validators,
				&network,
				externalized_value,
				externalized_n_h,
				expected_tx_set_hash,
				// used to check if all envelopes are using the same slot index
				externalized_envelope.statement.slot_index,
			)?;

			// ---- Check that externalized messages build valid quorum set ----
			check_for_valid_quorum_set(envelopes, validators, organizations.len())
		}

		pub(crate) fn get_tx_set_hash(scp_value: &Value) -> Result<Hash, Error<T>> {
			let tx_set_hash = StellarValue::from_xdr(scp_value.get_vec())
				.map(|stellar_value| stellar_value.tx_set_hash)
				.map_err(|_| Error::<T>::TransactionSetHashCreationFailed)?;
			Ok(tx_set_hash)
		}

		pub fn construct_from_raw_encoded_xdr<V: XdrCodec>(
			raw_encoded_xdr: &[u8],
		) -> Result<V, Error<T>> {
			let value_xdr =
				base64::decode(raw_encoded_xdr).map_err(|_| Error::<T>::Base64DecodeError)?;
			let decoded = V::from_xdr(value_xdr).map_err(|_| Error::<T>::InvalidXDR)?;
			Ok(decoded)
		}

		pub fn ensure_transaction_memo_matches_hash(
			transaction_envelope: &TransactionEnvelope,
			expected_hash: &H256,
		) -> Result<(), Error<T>> {
			let expected_memo = derive_shortened_request_id(&expected_hash.0);
			Self::ensure_transaction_memo_matches(transaction_envelope, &expected_memo)
		}

		pub fn ensure_transaction_memo_matches(
			transaction_envelope: &TransactionEnvelope,
			expected_memo: &TextMemo,
		) -> Result<(), Error<T>> {
			let tx_memo_text = get_text_memo_from_tx_env(transaction_envelope);

			if let Some(included_memo) = tx_memo_text {
				ensure!(included_memo == expected_memo, Error::TransactionMemoDoesNotMatch);
			} else {
				return Err(Error::TransactionMemoDoesNotMatch);
			}

			Ok(())
		}
	}

	pub(crate) fn verify_signature(
		envelope: &ScpEnvelope,
		node_id: &NodeId,
		network: &Network,
	) -> bool {
		let mut vec: [u8; 64] = [0; 64];
		vec.copy_from_slice(envelope.signature.get_vec());
		let signature: &primitives::stellar::Signature = &vec;

		// Envelope_Type_SCP = 1, see https://github.dev/stellar/stellar-core/blob/d3b80614cb92f44b789ac79f3dee29ca09de6fdb/src/protocol-curr/xdr/Stellar-ledger-entries.x#L586
		let envelope_type_scp: Vec<u8> = [0, 0, 0, 1].to_vec(); // xdr representation
		let network = network.get_id();

		// Signature is created by signing concatenation of network id, SCP type and SCP statement
		let body: Vec<u8> =
			[network.to_vec(), envelope_type_scp, envelope.statement.to_xdr()].concat();

		node_id.verify_signature(body, signature)
	}

	// Used to create bounded vecs for genesis config
	// Does not return a result but panics because the genesis config is hardcoded
	fn create_bounded_vec(input: &str) -> BoundedVec<u8, FieldLength> {
		let bounded_vec = BoundedVec::try_from(input.as_bytes().to_vec());

		assert!(bounded_vec.is_ok());
		bounded_vec.unwrap()
	}
}
