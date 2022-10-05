use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::Get, BoundedVec, RuntimeDebug};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

pub struct FieldLength;

impl Get<u32> for FieldLength {
	fn get() -> u32 {
		128
	}
}

pub type OrganizationID = u128;

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	MaxEncodedLen,
	Clone,
	Eq,
	PartialEq,
	Serialize,
	Deserialize,
	TypeInfo,
)]
pub struct Validator {
	pub name: BoundedVec<u8, FieldLength>,
	pub public_key: BoundedVec<u8, FieldLength>,
	pub organization_id: OrganizationID,
}

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	MaxEncodedLen,
	Clone,
	Eq,
	PartialEq,
	Serialize,
	Deserialize,
	TypeInfo,
)]
pub struct Organization {
	pub id: OrganizationID,
	pub name: BoundedVec<u8, FieldLength>,
}
