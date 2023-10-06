use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::Get, BoundedVec, RuntimeDebug};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use substrate_stellar_sdk::{
	compound_types::UnlimitedVarArray,
	types::{GeneralizedTransactionSet, TransactionSet},
	Hash, TransactionEnvelope,
};

pub struct FieldLength;

impl Get<u32> for FieldLength {
	fn get() -> u32 {
		128
	}
}

#[derive(
	Clone, Decode, Encode, Eq, MaxEncodedLen, Ord, PartialEq, PartialOrd, RuntimeDebug, TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Validator<OrganizationId> {
	pub name: BoundedVec<u8, FieldLength>,
	pub public_key: BoundedVec<u8, FieldLength>,
	pub organization_id: OrganizationId,
}

#[derive(Clone, Decode, Encode, Eq, MaxEncodedLen, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Organization<OrganizationId> {
	pub id: OrganizationId,
	pub name: BoundedVec<u8, FieldLength>,
}

pub enum TransactionSetType {
	TransactionSet(TransactionSet),
	GeneralizedTransactionSet(GeneralizedTransactionSet),
}

impl TransactionSetType {
	pub fn get_tx_set_hash(&self) -> Result<Hash, ()> {
		use substrate_stellar_sdk::IntoHash;
		match self {
			TransactionSetType::TransactionSet(tx_set) =>
				tx_set.clone().into_hash().map_err(|_| ()),
			TransactionSetType::GeneralizedTransactionSet(tx_set) =>
				tx_set.clone().into_hash().map_err(|_| ()),
			_ => Err(()),
		}
	}

	pub fn txes(&self) -> UnlimitedVarArray<TransactionEnvelope> {
		let txes_option = match self {
			TransactionSetType::TransactionSet(tx_set) => Some(tx_set.txes.clone()),
			TransactionSetType::GeneralizedTransactionSet(tx_set) => tx_set.txes(),
			_ => None,
		};

		txes_option.unwrap_or_else(UnlimitedVarArray::new_empty)
	}
}
