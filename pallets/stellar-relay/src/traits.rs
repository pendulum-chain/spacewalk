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
	pub organization: BoundedVec<u8, FieldLength>,
	pub total_org_nodes: u32,
}

#[derive(Debug, Encode, Decode, Clone, TypeInfo, Eq, PartialEq)]
pub struct ExternalizedMessage {
	pub message: Vec<u8>,
	pub signature: Vec<u8>,
}
