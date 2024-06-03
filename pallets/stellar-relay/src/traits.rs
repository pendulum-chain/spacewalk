use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::Get, BoundedVec, pallet_prelude::RuntimeDebug};
use scale_info::TypeInfo;
use sp_runtime::serde::{Deserialize, Serialize};

pub struct FieldLength;

impl Get<u32> for FieldLength {
	fn get() -> u32 {
		128
	}
}

#[derive(
	Clone, Decode, Encode, Eq, MaxEncodedLen, Ord, PartialEq, PartialOrd, RuntimeDebug, TypeInfo, Serialize, Deserialize,
)]
pub struct Validator<OrganizationId> {
	pub name: BoundedVec<u8, FieldLength>,
	pub public_key: BoundedVec<u8, FieldLength>,
	pub organization_id: OrganizationId,
}

#[derive(Clone, Decode, Encode, Eq, MaxEncodedLen, PartialEq, RuntimeDebug, TypeInfo, Serialize, Deserialize)]
pub struct Organization<OrganizationId> {
	pub id: OrganizationId,
	pub name: BoundedVec<u8, FieldLength>,
}
