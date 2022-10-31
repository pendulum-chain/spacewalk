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

pub mod traits;
pub mod types;

mod default_weights;

#[frame_support::pallet]
pub mod pallet {
	use codec::FullCodec;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sha2::{Digest, Sha256};
	use sp_std::{collections::btree_map::BTreeMap, fmt::Debug, vec::Vec};
	use substrate_stellar_sdk::{
		compound_types::UnlimitedVarArray,
		network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
		types::{
			NodeId, OperationBody, ScpEnvelope, ScpStatementExternalize, ScpStatementPledges,
			StellarValue, TransactionSet, Uint256,
		},
		Asset, Hash, MuxedAccount, PublicKey, TransactionEnvelope, XdrCodec,
	};

	use currency::CurrencyId;
	use default_weights::WeightInfo;

	use crate::{
		traits::FieldLength,
		types::{OrganizationOf, ValidatorOf},
	};

	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + currency::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

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

		type WeightInfo: WeightInfo;
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
		InvalidQuorumSetNotEnoughOrganizations,
		InvalidQuorumSetNotEnoughValidators,
		InvalidScpPledge,
		InvalidEnvelopeSignature,
		InvalidXDR,
		NoOrganizationsRegisteredForNetwork,
		NoValidatorsRegisteredForNetwork,
		InvalidTransactionSet,
		InvalidTransactionXDR,
		NoOrganizationsRegistered,
		NoValidatorsRegistered,
		OrganizationLimitExceeded,
		TransactionNotInTransactionSet,
		TransactionSetHashCreationFailed,
		TransactionSetHashMismatch,
		TryIntoError,
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
	#[pallet::getter(fn is_public_network)]
	pub type IsPublicNetwork<T: Config> = StorageValue<_, bool, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub validators: Vec<ValidatorOf<T>>,
		pub organizations: Vec<OrganizationOf<T>>,
		pub is_public_network: bool,
		pub phantom: PhantomData<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			// Create public network organizations
			let organization_sdf = OrganizationOf::<T> {
				name: create_bounded_vec("Stellar Development Foundation"),
				id: 0.into(),
			};
			let organization_satoshipay =
				OrganizationOf::<T> { name: create_bounded_vec("SatoshiPay"), id: 1.into() };
			let organization_wirex =
				OrganizationOf::<T> { name: create_bounded_vec("Wirex"), id: 2.into() };
			let organization_coinqvest =
				OrganizationOf::<T> { name: create_bounded_vec("Coinqvest"), id: 3.into() };
			let organization_blockdaemon =
				OrganizationOf::<T> { name: create_bounded_vec("Blockdaemon"), id: 4.into() };
			let organization_lobstr =
				OrganizationOf::<T> { name: create_bounded_vec("LOBSTR"), id: 5.into() };
			let organization_public_node =
				OrganizationOf::<T> { name: create_bounded_vec("Public Node"), id: 6.into() };

			let validators: Vec<ValidatorOf<T>> = vec![
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
				// Wirex validators
				ValidatorOf::<T> {
					name: create_bounded_vec("$wirex-sg"),
					public_key: create_bounded_vec(
						"GAB3GZIE6XAYWXGZUDM4GMFFLJBFMLE2JDPUCWUZXMOMT3NHXDHEWXAS",
					),
					organization_id: organization_wirex.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$wirex-us"),
					public_key: create_bounded_vec(
						"GDXUKFGG76WJC7ACEH3JUPLKM5N5S76QSMNDBONREUXPCZYVPOLFWXUS",
					),
					organization_id: organization_wirex.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$wirex-uk"),
					public_key: create_bounded_vec(
						"GBBQQT3EIUSXRJC6TGUCGVA3FVPXVZLGG3OJYACWBEWYBHU46WJLWXEU",
					),
					organization_id: organization_wirex.id,
				},
				// Coinqvest validators
				ValidatorOf::<T> {
					name: create_bounded_vec("$coinqvest-germany"),
					public_key: create_bounded_vec(
						"GD6SZQV3WEJUH352NTVLKEV2JM2RH266VPEM7EH5QLLI7ZZAALMLNUVN",
					),
					organization_id: organization_coinqvest.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$coinqvest-finland"),
					public_key: create_bounded_vec(
						"GADLA6BJK6VK33EM2IDQM37L5KGVCY5MSHSHVJA4SCNGNUIEOTCR6J5T",
					),
					organization_id: organization_coinqvest.id,
				},
				ValidatorOf::<T> {
					name: create_bounded_vec("$coinqvest-hongkong"),
					public_key: create_bounded_vec(
						"GAZ437J46SCFPZEDLVGDMKZPLFO77XJ4QVAURSJVRZK2T5S7XUFHXI2Z",
					),
					organization_id: organization_coinqvest.id,
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
						"GDXQB3OMMQ6MGG43PWFBZWBFKBBDUZIVSUDAZZTRAWQZKES2CDSE5HKJ",
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
					name: create_bounded_vec("$boötes"),
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
			];

			let organizations: Vec<OrganizationOf<T>> = vec![
				organization_satoshipay,
				organization_sdf,
				organization_wirex,
				organization_coinqvest,
				organization_blockdaemon,
				organization_lobstr,
				organization_public_node,
				organization_testnet_sdf,
			];

			GenesisConfig {
				validators,
				organizations,
				is_public_network: true,
				phantom: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let validator_vec =
				BoundedVec::<ValidatorOf<T>, T::ValidatorLimit>::try_from(self.validators.clone());
			assert!(validator_vec.is_ok());
			Validators::<T>::put(validator_vec.unwrap());

			let organization_vec = BoundedVec::<OrganizationOf<T>, T::OrganizationLimit>::try_from(
				self.organizations.clone(),
			);
			assert!(organization_vec.is_ok());
			Organizations::<T>::put(organization_vec.unwrap());

			IsPublicNetwork::<T>::put(self.is_public_network);
		}
	}

	// Extrinsics
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// This extrinsic is used to update/replace the current sets of validators and
		/// organizations
		#[pallet::weight(<T as Config>::WeightInfo::update_tier_1_validator_set())]
		pub fn update_tier_1_validator_set(
			origin: OriginFor<T>,
			validators: Vec<ValidatorOf<T>>,
			organizations: Vec<OrganizationOf<T>>,
		) -> DispatchResult {
			// Limit this call to root
			let _ = ensure_root(origin)?;

			Self::_update_tier_1_validator_set(validators, organizations)
		}
	}

	// Helper functions
	impl<T: Config> Pallet<T> {
		pub fn _update_tier_1_validator_set(
			validators: Vec<ValidatorOf<T>>,
			organizations: Vec<OrganizationOf<T>>,
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

			let validator_vec =
				BoundedVec::<ValidatorOf<T>, T::ValidatorLimit>::try_from(validators)
					.map_err(|_| Error::<T>::BoundedVecCreationFailed)?;
			Validators::<T>::put(validator_vec);

			let organization_vec =
				BoundedVec::<OrganizationOf<T>, T::OrganizationLimit>::try_from(organizations)
					.map_err(|_| Error::<T>::BoundedVecCreationFailed)?;
			Organizations::<T>::put(organization_vec);

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
			transaction_envelope: &TransactionEnvelope,
			envelopes: &UnlimitedVarArray<ScpEnvelope>,
			transaction_set: &TransactionSet,
		) -> Result<(), Error<T>> {
			let network: &Network =
				if Self::is_public_network() { &PUBLIC_NETWORK } else { &TEST_NETWORK };

			// Check if tx is included in the transaction set
			let tx_hash = transaction_envelope.get_hash(&network);
			let tx_included =
				transaction_set.txes.get_vec().iter().any(|tx| tx.get_hash(&network) == tx_hash);
			ensure!(tx_included, Error::<T>::TransactionNotInTransactionSet);

			// Check if all externalized ScpEnvelopes were signed by a tier 1 validator
			let validators = Validators::<T>::get();
			// Make sure that at least one validator is registered
			ensure!(!validators.is_empty(), Error::<T>::NoValidatorsRegistered);

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
			// Find the validators that are targeted by the SCP messages
			let targeted_validators = validators
				.iter()
				.filter(|validator| {
					envelopes.get_vec().iter().any(|envelope| {
						envelope.statement.node_id.to_encoding() == validator.public_key.to_vec()
					})
				})
				.collect::<Vec<&ValidatorOf<T>>>();

			let organizations = Organizations::<T>::get();
			// Make sure that at least one organization is registered
			ensure!(!organizations.is_empty(), Error::<T>::NoOrganizationsRegistered);

			// Map organizationID to the number of validators that belongs to it
			let mut validator_count_per_organization_map =
				BTreeMap::<T::OrganizationId, u32>::new();
			for validator in validators.iter() {
				validator_count_per_organization_map
					.entry(validator.organization_id)
					.and_modify(|e| {
						*e += 1;
					})
					.or_insert(1);
			}

			// Build a map used to identify the targeted organizations
			// A map is used to avoid duplicates and simultaneously track the number of validators
			// that were targeted
			let mut targeted_organization_map = BTreeMap::<T::OrganizationId, u32>::new();
			for validator in targeted_validators {
				targeted_organization_map
					.entry(validator.organization_id)
					.and_modify(|e| {
						*e += 1;
					})
					.or_insert(1);
			}

			// Count the number of distinct organizations that are targeted by the SCP messages
			let targeted_organization_count = targeted_organization_map.len();

			// Check that the distinct organizations occurring in the validator structs related to
			// the externalized messages are more than 2/3 of the total amount of organizations in
			// the tier 1 validator set.
			// Use multiplication to avoid floating point numbers.
			ensure!(
				targeted_organization_count * 3 > organizations.len() * 2,
				Error::<T>::InvalidQuorumSetNotEnoughOrganizations
			);

			for (organization_id, count) in targeted_organization_map.iter() {
				let total: &u32 =
					validator_count_per_organization_map.get(organization_id).unwrap();
				// Check that for each of the targeted organizations more than 1/2 of their total
				// validators were used in the SCP messages
				ensure!(count * 2 > *total, Error::<T>::InvalidQuorumSetNotEnoughValidators);
			}

			Ok(())
		}

		fn get_tx_set_hash(x: &ScpStatementExternalize) -> Result<Hash, Error<T>> {
			let scp_value = x.commit.value.get_vec();
			let tx_set_hash = StellarValue::from_xdr(scp_value)
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

		/// Accumulate the amounts of the specified currency that happened in the operations of a
		/// Stellar transaction
		pub fn get_amount_from_transaction_envelope<V: TryFrom<i64>>(
			transaction_envelope: &TransactionEnvelope,
			recipient_stellar_address: Uint256,
			currency: &CurrencyId<T>,
		) -> Result<V, Error<T>> {
			// TODO derive asset from currency and back
			let asset = Asset::AssetTypeNative;
			let recipient_account = MuxedAccount::KeyTypeEd25519(recipient_stellar_address);

			let amount: i64 = match transaction_envelope {
				TransactionEnvelope::EnvelopeTypeTxV0(envelope) => {
					let mut sum: i64 = 0;
					for x in envelope.tx.operations.get_vec().iter() {
						if let OperationBody::Payment(payment) = x.body.clone() {
							if payment.destination.eq(&recipient_account) && payment.asset == asset
							{
								sum = sum.saturating_add(payment.amount);
							}
						}
					}
					sum
				},
				TransactionEnvelope::EnvelopeTypeTx(envelope) => {
					let mut sum: i64 = 0;
					for x in envelope.tx.operations.get_vec().iter() {
						if let OperationBody::Payment(payment) = x.body.clone() {
							if payment.destination.eq(&recipient_account) && payment.asset == asset
							{
								sum = sum.saturating_add(payment.amount);
							}
						}
					}
					sum
				},
				TransactionEnvelope::EnvelopeTypeTxFeeBump(_) => 0,
				TransactionEnvelope::Default(_) => 0,
			};

			let amount: V = amount.try_into().map_err(|_| Error::<T>::TryIntoError)?;
			Ok(amount)
		}
	}

	pub fn compute_non_generic_tx_set_content_hash(tx_set: &TransactionSet) -> [u8; 32] {
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

	// Used to create bounded vecs for genesis config
	// Does not return a result but panics because the genesis config is hardcoded
	fn create_bounded_vec(input: &str) -> BoundedVec<u8, FieldLength> {
		let bounded_vec =
			BoundedVec::try_from(input.as_bytes().to_vec()).expect("Failed to create bounded vec");

		bounded_vec
	}
}
