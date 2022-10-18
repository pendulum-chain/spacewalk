#![cfg_attr(not(feature = "std"), no_std)]

pub mod currency;

pub use sp_core::H256;
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	FixedI128, FixedPointNumber, FixedU128, MultiSignature,
};
use sp_std::convert::TryInto;

pub trait BalanceToFixedPoint<FixedPoint> {
	fn to_fixed(self) -> Option<FixedPoint>;
}

impl BalanceToFixedPoint<SignedFixedPoint> for Balance {
	fn to_fixed(self) -> Option<SignedFixedPoint> {
		SignedFixedPoint::checked_from_integer(
			TryInto::<<SignedFixedPoint as FixedPointNumber>::Inner>::try_into(self).ok()?,
		)
	}
}

pub trait TruncateFixedPointToInt: FixedPointNumber {
	/// take a fixed point number and turns it into the truncated inner representation,
	/// e.g. FixedU128(1.23) -> 1u128
	fn truncate_to_inner(&self) -> Option<<Self as FixedPointNumber>::Inner>;
}

impl TruncateFixedPointToInt for SignedFixedPoint {
	fn truncate_to_inner(&self) -> Option<Self::Inner> {
		self.into_inner().checked_div(SignedFixedPoint::accuracy())
	}
}

impl TruncateFixedPointToInt for UnsignedFixedPoint {
	fn truncate_to_inner(&self) -> Option<<Self as FixedPointNumber>::Inner> {
		self.into_inner().checked_div(UnsignedFixedPoint::accuracy())
	}
}

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
/// never know...
pub type AccountIndex = u32;

/// Index of a transaction in the chain. 32-bit should be plenty.
pub type Nonce = u32;

/// Balance of an account.
pub type Balance = u128;

/// Signed version of Balance
pub type Amount = i128;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// An instant or duration in time.
pub type Moment = u64;

/// Opaque block header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Opaque block type.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// Opaque block identifier type.
pub type BlockId = generic::BlockId<Block>;

/// The signed fixed point type.
pub type SignedFixedPoint = FixedI128;

/// The `Inner` type of the `SignedFixedPoint`.
pub type SignedInner = i128;

/// The unsigned fixed point type.
pub type UnsignedFixedPoint = FixedU128;

/// The `Inner` type of the `UnsignedFixedPoint`.
pub type UnsignedInner = u128;
