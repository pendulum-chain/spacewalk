use frame_support::{ensure, BoundedVec};
use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

use substrate_stellar_sdk::{
	compound_types::UnlimitedVarArray,
	network::Network,
	types::{NodeId, ScpEnvelope, ScpStatementPledges, Value},
	Hash,
};

use crate::{
	pallet::{verify_signature, Config},
	types::{OrganizationOf, ValidatorOf},
	Error, NewValidatorsEnactmentBlockHeight, OldOrganizations, OldValidators, Organizations,
	Pallet, Validators,
};

/// Returns a map of organizationID to the number of validators that belongs to it
fn validator_count_per_org<T: Config>(
	validators: &BoundedVec<ValidatorOf<T>, T::ValidatorLimit>,
) -> BTreeMap<T::OrganizationId, u32> {
	let mut validator_count_per_organization_map = BTreeMap::<T::OrganizationId, u32>::new();

	for validator in validators.iter() {
		validator_count_per_organization_map
			.entry(validator.organization_id)
			.and_modify(|e| {
				*e += 1;
			})
			.or_insert(1);
	}

	validator_count_per_organization_map
}

/// Builds a map used to identify the targeted organizations
fn targeted_organization_map<T: Config>(
	envelopes: &UnlimitedVarArray<ScpEnvelope>,
	validators: &BoundedVec<ValidatorOf<T>, T::ValidatorLimit>,
	orgs_length: usize,
) -> Result<BTreeMap<T::OrganizationId, u32>, Error<T>> {
	// Find the validators that are targeted by the SCP messages
	let targeted_validators = validators
		.iter()
		.filter(|validator| {
			envelopes.get_vec().iter().any(|envelope| {
				envelope.statement.node_id.to_encoding() == validator.public_key.to_vec()
			})
		})
		.collect::<Vec<&ValidatorOf<T>>>();

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
		targeted_organization_count * 3 > orgs_length * 2,
		Error::<T>::InvalidQuorumSetNotEnoughOrganizations
	);

	Ok(targeted_organization_map)
}

/// Returns a tuple of Externalized Value and Externalized n_h
pub fn get_externalized_info<T: Config>(envelope: &ScpEnvelope) -> Result<(&Value, u32), Error<T>> {
	match &envelope.statement.pledges {
		ScpStatementPledges::ScpStExternalize(externalized_statement) =>
			Ok((&externalized_statement.commit.value, externalized_statement.n_h)),
		ScpStatementPledges::ScpStConfirm(confirmed_statement) =>
			Ok((&confirmed_statement.ballot.value, confirmed_statement.n_h)),
		_ => return Err(Error::<T>::InvalidScpPledge),
	}
}

/// Returns the node id of the envelope if it is part of the set of validators
fn get_node_id<T: Config>(
	envelope: &ScpEnvelope,
	validators: &BoundedVec<ValidatorOf<T>, T::ValidatorLimit>,
) -> Result<NodeId, Error<T>> {
	let node_id = envelope.statement.node_id.clone();
	let node_id_found = validators
		.iter()
		.any(|validator| validator.public_key.to_vec() == node_id.to_encoding());

	ensure!(node_id_found, Error::<T>::EnvelopeSignedByUnknownValidator);

	Ok(node_id)
}

pub fn check_targets<T: Config>(
	envelopes: &UnlimitedVarArray<ScpEnvelope>,
	validators: BoundedVec<ValidatorOf<T>, T::ValidatorLimit>,
	orgs_length: usize,
) -> Result<(), Error<T>> {
	let validator_count_per_organization_map = validator_count_per_org::<T>(&validators);

	let targeted_organization_map = targeted_organization_map(envelopes, &validators, orgs_length)?;

	for (organization_id, count) in targeted_organization_map.iter() {
		let total: &u32 = validator_count_per_organization_map
			.get(organization_id)
			.ok_or(Error::<T>::NoOrganizationsRegistered)?;
		// Check that for each of the targeted organizations more than 1/2 of their total
		// validators were used in the SCP messages
		ensure!(count * 2 > *total, Error::<T>::InvalidQuorumSetNotEnoughValidators);
	}

	Ok(())
}

/// Checks that all envelopes have the same values
///
/// # Arguments
///
/// * `envelopes` - The set of SCP envelopes that were externalized on the Stellar network
/// * `validators` - The set of validators allowed to sign envelopes
/// * `network` - used for verifying signatures
/// * `externalized_value` - A value that must be equal amongst all envelopes
/// * `externalized_n_h` - A value that must be equal amongst all envelopes
/// * `expected_tx_set_hash` - A value that must be equal amongst all envelopes
/// * `slot_index` - used to check if all envelopes are using the same slot
pub fn validate_envelopes<T: Config>(
	envelopes: &UnlimitedVarArray<ScpEnvelope>,
	validators: &BoundedVec<ValidatorOf<T>, T::ValidatorLimit>,
	network: &Network,
	externalized_value: &Value,
	externalized_n_h: u32,
	expected_tx_set_hash: Hash,
	slot_index: u64,
) -> Result<(), Error<T>> {
	let mut externalized_n_h = externalized_n_h;
	for envelope in envelopes.get_vec() {
		let node_id = get_node_id(envelope, validators)?;

		// Check if all envelopes are using the same slot index
		ensure!(slot_index == envelope.statement.slot_index, Error::<T>::EnvelopeSlotIndexMismatch);

		let signature_valid = verify_signature(envelope, &node_id, network);
		ensure!(signature_valid, Error::<T>::InvalidEnvelopeSignature);

		let (value, n_h) = get_externalized_info(envelope)?;

		// Check if the tx_set_hash matches the one included in the envelope
		let tx_set_hash = Pallet::<T>::get_tx_set_hash(&value)?;
		ensure!(tx_set_hash == expected_tx_set_hash, Error::<T>::TransactionSetHashMismatch);

		// Check if the externalized value is the same for all envelopes
		ensure!(externalized_value == value, Error::<T>::ExternalizedValueMismatch);

		// use this envelopes's n_h as basis for the comparison with the succeeding
		// envelopes
		if externalized_n_h == u32::MAX {
			externalized_n_h = n_h;
		}
		// check for equality of n_h values
		// that are not 'infinity' (represented internally by `u32::MAX`)
		else if n_h < u32::MAX {
			ensure!(externalized_n_h == n_h, Error::<T>::ExternalizedNHMismatch);
		}
	}

	Ok(())
}

pub fn find_externalized_envelope<T: Config>(
	envelopes: &UnlimitedVarArray<ScpEnvelope>,
) -> Result<&ScpEnvelope, Error<T>> {
	envelopes
		.get_vec()
		.iter()
		.find(|envelope| match envelope.statement.pledges {
			ScpStatementPledges::ScpStExternalize(_) => true,
			_ => false,
		})
		.ok_or(Error::<T>::MissingExternalizedMessage)
}

pub fn validators_and_orgs<T: Config>() -> Result<
	(
		BoundedVec<ValidatorOf<T>, T::ValidatorLimit>,
		BoundedVec<OrganizationOf<T>, T::OrganizationLimit>,
	),
	Error<T>,
> {
	// Choose the set of validators to use for validation based on the enactment block
	// height and the current block number
	let should_use_new_validator_set =
		<frame_system::Pallet<T>>::block_number() >= NewValidatorsEnactmentBlockHeight::<T>::get();
	let validators = if should_use_new_validator_set {
		Validators::<T>::get()
	} else {
		OldValidators::<T>::get()
	};

	// Make sure that at least one validator is registered
	ensure!(!validators.is_empty(), Error::<T>::NoValidatorsRegistered);

	// Choose the set of organizations to use for validation based on the enactment block
	// height and the current block number
	let organizations = if should_use_new_validator_set {
		Organizations::<T>::get()
	} else {
		OldOrganizations::<T>::get()
	};
	// Make sure that at least one organization is registered
	ensure!(!organizations.is_empty(), Error::<T>::NoOrganizationsRegistered);

	Ok((validators, organizations))
}
