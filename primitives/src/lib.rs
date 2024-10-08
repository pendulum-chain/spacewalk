#![cfg_attr(not(feature = "std"), no_std)]
#![allow(non_upper_case_globals)]

use base58::ToBase58;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::error::LookupError;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use serde::{Deserializer, Serializer};
pub use sp_core::H256;
use sp_core::{crypto::AccountId32, ed25519};
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, CheckedDiv, CheckedMul, Convert, IdentifyAccount, StaticLookup, Verify},
	FixedI128, FixedPointNumber, FixedU128, MultiSignature, MultiSigner,
};
use sp_std::{
	convert::{From, TryFrom, TryInto},
	fmt,
	prelude::*,
	str,
	str::from_utf8,
	vec::Vec,
};
use stellar::{
	types::{AlphaNum12, AlphaNum4},
	Asset as StellarAsset, PublicKey,
};
pub use substrate_stellar_sdk as stellar;
use substrate_stellar_sdk::{
	types::{OperationBody, SequenceNumber},
	ClaimPredicate, Claimant, Memo, MuxedAccount, Operation, Transaction, TransactionEnvelope,
};

#[cfg(test)]
mod tests;

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

#[derive(
	Encode,
	Decode,
	Clone,
	PartialEq,
	Eq,
	Debug,
	PartialOrd,
	Ord,
	TypeInfo,
	MaxEncodedLen,
	Deserialize,
	Serialize,
)]
#[cfg_attr(feature = "std", derive(std::hash::Hash))]
pub struct VaultCurrencyPair<CurrencyId: Copy> {
	pub collateral: CurrencyId,
	pub wrapped: CurrencyId,
}

#[derive(
	Encode,
	Decode,
	Clone,
	PartialEq,
	Eq,
	Debug,
	PartialOrd,
	Ord,
	TypeInfo,
	MaxEncodedLen,
	Serialize,
	Deserialize,
)]
#[cfg_attr(feature = "std", derive(std::hash::Hash))]
pub struct VaultId<AccountId, CurrencyId: Copy> {
	pub account_id: AccountId,
	pub currencies: VaultCurrencyPair<CurrencyId>,
}

impl<AccountId, CurrencyId: Copy> VaultId<AccountId, CurrencyId> {
	pub fn new(
		account_id: AccountId,
		collateral_currency: CurrencyId,
		wrapped_currency: CurrencyId,
	) -> Self {
		Self {
			account_id,
			currencies: VaultCurrencyPair::<CurrencyId> {
				collateral: collateral_currency,
				wrapped: wrapped_currency,
			},
		}
	}

	pub fn collateral_currency(&self) -> CurrencyId {
		self.currencies.collateral
	}

	pub fn wrapped_currency(&self) -> CurrencyId {
		self.currencies.wrapped
	}
}

pub type StellarPublicKeyRaw = [u8; 32];

#[cfg(feature = "std")]
fn serialize_as_string<S: Serializer, T: std::fmt::Display>(
	t: &T,
	serializer: S,
) -> Result<S::Ok, S::Error> {
	serializer.serialize_str(&t.to_string())
}

#[cfg(feature = "std")]
fn deserialize_from_string<'de, D: Deserializer<'de>, T: std::str::FromStr>(
	deserializer: D,
) -> Result<T, D::Error> {
	let s = String::deserialize(deserializer)?;
	s.parse::<T>().map_err(|_| serde::de::Error::custom("Parse from string failed"))
}

pub mod issue {
	use super::*;

	#[derive(Default, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub enum IssueRequestStatus {
		/// opened, but not yet executed or cancelled
		#[default]
		Pending,
		/// payment was received
		Completed,
		/// payment was not received, vault may receive griefing collateral
		Cancelled,
	}

	// Due to a known bug in serde we need to specify how u128 is (de)serialized.
	// See https://github.com/paritytech/substrate/issues/4641
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	pub struct IssueRequest<AccountId, BlockNumber, Balance, CurrencyId: Copy> {
		/// the vault associated with this issue request
		pub vault: VaultId<AccountId, CurrencyId>,
		/// the *active* block height when this request was opened
		pub opentime: BlockNumber,
		/// the issue period when this request was opened
		pub period: BlockNumber,
		#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
		#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
		#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
		#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
		/// the collateral held for spam prevention
		pub griefing_collateral: Balance,
		#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
		#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
		#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
		#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
		/// the number of tokens that will be transferred to the user (as such, this does not
		/// include the fee)
		pub amount: Balance,
		pub asset: CurrencyId,
		#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
		#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
		#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
		#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
		/// the number of tokens that will be transferred to the fee pool
		pub fee: Balance,
		/// the account issuing tokens
		pub requester: AccountId,
		/// the vault's Stellar public key
		pub stellar_address: StellarPublicKeyRaw,
		/// the status of this issue request
		pub status: IssueRequestStatus,
	}
}

pub mod redeem {
	use super::*;

	#[derive(Default, Encode, Decode, Clone, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub enum RedeemRequestStatus {
		/// opened, but not yet executed or cancelled
		#[default]
		Pending,
		/// successfully executed with a valid payment from the vault
		Completed,
		/// bool=true indicates that the vault minted tokens for the amount that the redeemer
		/// burned
		Reimbursed(bool),
		/// user received compensation, but is retrying the redeem with another vault
		Retried,
	}

	// Due to a known bug in serde we need to specify how u128 is (de)serialized.
	// See https://github.com/paritytech/substrate/issues/4641
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	pub struct RedeemRequest<AccountId, BlockNumber, Balance, CurrencyId: Copy> {
		/// the vault associated with this redeem request
		pub vault: VaultId<AccountId, CurrencyId>,
		/// the *active* block height when this request was opened
		pub opentime: BlockNumber,
		/// the redeem period when this request was opened
		pub period: BlockNumber,
		#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
		#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
		#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
		#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
		/// total redeem fees - taken from request amount
		pub fee: Balance,
		#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
		#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
		#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
		#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
		/// amount the vault should spend on the Stellar inclusion fee - taken from request amount
		pub transfer_fee: Balance,
		#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
		#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
		#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
		#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
		/// total amount of some asset for the vault to send
		pub amount: Balance,
		pub asset: CurrencyId,
		#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
		#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
		#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
		#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
		/// premium redeem amount in collateral
		pub premium: Balance,
		/// the account redeeming tokens (for Stellar assets)
		pub redeemer: AccountId,
		/// the user's Stellar address for payment verification
		pub stellar_address: StellarPublicKeyRaw,
		/// the status of this redeem request
		pub status: RedeemRequestStatus,
	}
}

pub mod replace {
	use super::*;

	#[derive(Default, Encode, Decode, Clone, PartialEq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize, Eq))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub enum ReplaceRequestStatus {
		/// accepted, but not yet executed or cancelled
		#[default]
		Pending,
		/// successfully executed with a valid payment from the old vault
		Completed,
		/// payment was not received, new vault may receive griefing collateral
		Cancelled,
	}

	// Due to a known bug in serde we need to specify how u128 is (de)serialized.
	// See https://github.com/paritytech/substrate/issues/4641
	#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize, Eq))]
	pub struct ReplaceRequest<AccountId, BlockNumber, Balance, CurrencyId: Copy> {
		/// the vault which has requested to be replaced
		pub old_vault: VaultId<AccountId, CurrencyId>,
		/// the vault which is replacing the old vault
		pub new_vault: VaultId<AccountId, CurrencyId>,
		#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
		#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
		#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
		#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
		/// the amount of tokens to be replaced
		pub amount: Balance,
		pub asset: CurrencyId,
		#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
		#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
		#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
		#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
		/// the collateral held for spam prevention
		pub griefing_collateral: Balance,
		#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
		#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
		#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
		#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
		/// additional collateral to cover replacement
		pub collateral: Balance,
		/// the *active* block height when this request was opened
		pub accept_time: BlockNumber,
		/// the replace period when this request was opened
		pub period: BlockNumber,
		/// the Stellar address of the new vault
		pub stellar_address: StellarPublicKeyRaw,
		/// the status of this replace request
		pub status: ReplaceRequestStatus,
	}
}

pub mod oracle {
	use super::*;

	#[derive(
		Encode, Decode, Clone, Eq, PartialEq, Debug, TypeInfo, MaxEncodedLen, Serialize, Deserialize,
	)]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub enum Key {
		ExchangeRate(CurrencyId),
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

/// Balance of an account.
pub type Balance = u128;

/// Signed version of Balance
pub type Amount = i128;

/// Nonce of a transaction in the chain.
pub type Nonce = u32;

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

/// The type of a Stellar transaction text memo
pub type TextMemo = Vec<u8>;

pub trait MemoTypeExt {
	const TYPE_MEMOTEXT: &'static str;
	const TYPE_MEMOHASH: &'static str;

	fn is_type_text(memo_type_as_ref: &[u8]) -> bool;
	fn is_type_hash(memo_type_as_ref: &[u8]) -> bool;
}

impl MemoTypeExt for Memo {
	const TYPE_MEMOTEXT: &'static str = "text";
	const TYPE_MEMOHASH: &'static str = "hash";

	fn is_type_text(memo_type_as_ref: &[u8]) -> bool {
		memo_type_as_ref == Self::TYPE_MEMOTEXT.as_bytes()
	}

	fn is_type_hash(memo_type_as_ref: &[u8]) -> bool {
		memo_type_as_ref == Self::TYPE_MEMOHASH.as_bytes()
	}
}

/// Shorten the request id so that it fits into a Stellar transaction text memo
pub fn derive_shortened_request_id(hash: &[u8; 32]) -> TextMemo {
	hash.to_base58().as_bytes()[..28].to_vec()
}

/// Returns issue memo of the given TransactionEnvelope, or None if the transaction does
/// not contain an issue memo
pub fn get_text_memo_from_tx_env(transaction_envelope: &TransactionEnvelope) -> Option<&TextMemo> {
	let memo_text = match transaction_envelope {
		TransactionEnvelope::EnvelopeTypeTxV0(tx_env) => match &tx_env.tx.memo {
			Memo::MemoText(text) => Some(text.get_vec()),
			_ => None,
		},
		TransactionEnvelope::EnvelopeTypeTx(tx_env) => match &tx_env.tx.memo {
			Memo::MemoText(text) => Some(text.get_vec()),
			_ => None,
		},
		_ => None,
	};

	memo_text
}

pub trait CurrencyInfo {
	fn name(&self) -> &str;
	fn symbol(&self) -> &str;
	fn decimals(&self) -> u8;
}

pub fn remove_trailing_non_alphanum_bytes(input: &[u8]) -> &[u8] {
	for (idx, elem) in input.iter().enumerate().rev() {
		if elem.is_ascii_alphanumeric() {
			return &input[..=idx];
		}
	}
	b""
}

#[derive(
	Encode,
	Decode,
	Eq,
	Hash,
	PartialEq,
	Copy,
	Clone,
	PartialOrd,
	Ord,
	TypeInfo,
	MaxEncodedLen,
	Serialize,
	Deserialize,
	scale_decode::DecodeAsType,
	scale_encode::EncodeAsType,
)]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[repr(u8)]
#[allow(clippy::unnecessary_cast)]
pub enum Asset {
	StellarNative = 0_u8,
	AlphaNum4 { code: Bytes4, issuer: AssetIssuer },
	AlphaNum12 { code: Bytes12, issuer: AssetIssuer },
}

impl CurrencyInfo for Asset {
	fn name(&self) -> &str {
		match self {
			Asset::StellarNative => "Stellar",
			Asset::AlphaNum4 { code, issuer: _ } =>
				from_utf8(remove_trailing_non_alphanum_bytes(code)).unwrap_or("unspecified"),
			Asset::AlphaNum12 { code, issuer: _ } =>
				from_utf8(remove_trailing_non_alphanum_bytes(code)).unwrap_or("unspecified"),
		}
	}

	fn symbol(&self) -> &str {
		match self {
			Asset::StellarNative => "XLM",
			_ => self.name(),
		}
	}

	fn decimals(&self) -> u8 {
		// We assume 12 decimals for all assets. To ensure that Stellar Assets also have 12 decimals
		// instead of the 7 decimals they have on Stellar, we use the BalanceConversion struct where
		// necessary, that is, when dealing with amounts contained in Stellar transactions.
		12
	}
}

impl Asset {
	pub fn one(&self) -> Balance {
		10u128.pow(self.decimals().into())
	}
}

#[derive(
	Default,
	Encode,
	Decode,
	Eq,
	Hash,
	PartialEq,
	Copy,
	Clone,
	PartialOrd,
	Ord,
	TypeInfo,
	MaxEncodedLen,
	Serialize,
	Deserialize,
	scale_decode::DecodeAsType,
	scale_encode::EncodeAsType,
)]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[repr(u8)]
#[allow(clippy::unnecessary_cast)]
pub enum CurrencyId {
	#[default]
	Native = 0_u8,
	XCM(u8),
	Stellar(Asset),
	ZenlinkLPToken(u8, u8, u8, u8),
	Token(u64),
}

pub trait DecimalsLookup {
	type CurrencyId;

	fn decimals(currency_id: Self::CurrencyId) -> u32;

	fn one(currency_id: Self::CurrencyId) -> Balance {
		10u128.pow(Self::decimals(currency_id))
	}
}

// We use the PendulumDecimalsLookup as the default implementation in Spacewalk because it
// is more interesting with the XCM(0) token having 10 decimals vs the default 12 decimals.
pub type DefaultDecimalsLookup = PendulumDecimalsLookup;

pub struct PendulumDecimalsLookup;
impl DecimalsLookup for PendulumDecimalsLookup {
	type CurrencyId = CurrencyId;

	fn decimals(currency_id: Self::CurrencyId) -> u32 {
		(match currency_id {
			CurrencyId::Stellar(asset) => asset.decimals(),
			CurrencyId::XCM(index) => match index {
				// DOT
				0 => 10,
				// Assethub USDT
				1 => 6,
				// Assethub USDC
				2 => 6,
				// EQD
				3 => 9,
				// Moonbeam BRZ
				4 => 18,
				// PDEX
				5 => 12,
				// GLMR
				6 => 18,
				// PINK
				7 => 10,
				// HDX
				8 => 12,
				// vDOT
				9 => 10,
				_ => 12,
			},
			// We assume that all other assets have 12 decimals
			CurrencyId::Native | CurrencyId::ZenlinkLPToken(_, _, _, _) | CurrencyId::Token(_) =>
				12,
		}) as u32
	}
}

pub struct AmplitudeDecimalsLookup;
impl DecimalsLookup for AmplitudeDecimalsLookup {
	type CurrencyId = CurrencyId;

	fn decimals(currency_id: Self::CurrencyId) -> u32 {
		(match currency_id {
			CurrencyId::Stellar(asset) => asset.decimals(),
			CurrencyId::XCM(index) => match index {
				// KSM
				0 => 12,
				// Assethub USDT
				1 => 6,
				_ => 12,
			},
			// We assume that all other assets have 12 decimals
			CurrencyId::Native | CurrencyId::ZenlinkLPToken(_, _, _, _) | CurrencyId::Token(_) =>
				12,
		}) as u32
	}
}

impl CurrencyId {
	pub const StellarNative: CurrencyId = Self::Stellar(Asset::StellarNative);

	#[allow(non_snake_case)]
	pub const fn AlphaNum4(code: Bytes4, issuer: AssetIssuer) -> Self {
		Self::Stellar(Asset::AlphaNum4 { code, issuer })
	}

	#[allow(non_snake_case)]
	pub const fn AlphaNum12(code: Bytes12, issuer: AssetIssuer) -> Self {
		Self::Stellar(Asset::AlphaNum12 { code, issuer })
	}
}

/// This struct defines the custom metadata for an asset registered on an asset registry pallet.
/// It's a unit struct because we don't need to store any metadata at the moment.
#[derive(scale_info::TypeInfo, Encode, Decode, Clone, Eq, PartialEq, Debug)]
pub struct CustomMetadata;

pub type Bytes4 = [u8; 4];
pub type Bytes12 = [u8; 12];
pub type AssetIssuer = [u8; 32];

impl TryFrom<(&str, &str)> for CurrencyId {
	type Error = &'static str;

	fn try_from(value: (&str, &str)) -> Result<Self, Self::Error> {
		let issuer_encoded = value.1;
		let issuer_pk = stellar::PublicKey::from_encoding(issuer_encoded)
			.map_err(|_| "Invalid issuer encoding")?;
		let issuer: AssetIssuer = *issuer_pk.as_binary();

		CurrencyId::try_from((value.0, issuer))
	}
}

impl TryFrom<(&str, AssetIssuer)> for CurrencyId {
	type Error = &'static str;

	fn try_from(value: (&str, AssetIssuer)) -> Result<Self, Self::Error> {
		let slice = value.0;
		let issuer = value.1;
		if slice.len() <= 4 {
			let mut code: Bytes4 = [0; 4];
			code[..slice.len()].copy_from_slice(slice.as_bytes());
			Ok(CurrencyId::AlphaNum4(code, issuer))
		} else if slice.len() <= 12 {
			let mut code: Bytes12 = [0; 12];
			code[..slice.len()].copy_from_slice(slice.as_bytes());
			Ok(CurrencyId::AlphaNum12(code, issuer))
		} else {
			Err("More than 12 bytes not supported")
		}
	}
}

impl From<stellar::Asset> for CurrencyId {
	fn from(asset: stellar::Asset) -> Self {
		match asset {
			StellarAsset::Default(_) => CurrencyId::StellarNative,
			StellarAsset::AssetTypeNative => CurrencyId::StellarNative,
			StellarAsset::AssetTypeCreditAlphanum4(asset_alpha_num4) => CurrencyId::AlphaNum4(
				asset_alpha_num4.asset_code,
				asset_alpha_num4.issuer.into_binary(),
			),
			StellarAsset::AssetTypeCreditAlphanum12(asset_alpha_num12) => CurrencyId::AlphaNum12(
				asset_alpha_num12.asset_code,
				asset_alpha_num12.issuer.into_binary(),
			),
		}
	}
}

impl TryInto<stellar::Asset> for CurrencyId {
	type Error = &'static str;

	fn try_into(self) -> Result<stellar::Asset, Self::Error> {
		match self {
			Self::XCM(_currency_id) => Err("XCM Foreign Asset not defined in the Stellar world."),
			Self::Native => Err("PEN token not defined in the Stellar world."),
			Self::StellarNative => Ok(stellar::Asset::native()),
			Self::Stellar(Asset::AlphaNum4 { code, issuer }) =>
				Ok(stellar::Asset::AssetTypeCreditAlphanum4(AlphaNum4 {
					asset_code: code,
					issuer: PublicKey::PublicKeyTypeEd25519(issuer),
				})),
			Self::Stellar(Asset::AlphaNum12 { code, issuer }) =>
				Ok(stellar::Asset::AssetTypeCreditAlphanum12(AlphaNum12 {
					asset_code: code,
					issuer: PublicKey::PublicKeyTypeEd25519(issuer),
				})),
			Self::ZenlinkLPToken(_, _, _, _) =>
				Err("Zenlink LP Token not defined in the Stellar world."),
			Self::Token(_) => Err("Token not defined in the Stellar world."),
		}
	}
}

impl fmt::Debug for CurrencyId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::XCM(id) => {
				write!(f, "XCM({:?})", id)
			},
			Self::Native => write!(f, "Native"),
			&Self::StellarNative => write!(f, "XLM"),
			Self::Stellar(Asset::AlphaNum4 { code, issuer }) => {
				write!(
					f,
					"{{ code: {}, issuer: {} }}",
					str::from_utf8(code).unwrap_or_default(),
					str::from_utf8(
						stellar::PublicKey::from_binary(*issuer).to_encoding().as_slice()
					)
					.unwrap_or_default()
				)
			},
			Self::Stellar(Asset::AlphaNum12 { code, issuer }) => {
				write!(
					f,
					"{{ code: {}, issuer: {} }}",
					str::from_utf8(code).unwrap_or_default(),
					str::from_utf8(
						stellar::PublicKey::from_binary(*issuer).to_encoding().as_slice()
					)
					.unwrap_or_default()
				)
			},
			&Self::ZenlinkLPToken(token1_id, token1_type, token2_id, token2_type) => {
				write!(
					f,
					"{{ token1 id: {}, token1 type: {}, token2 id: {}, token2 type: {} }}",
					token1_id, token1_type, token2_id, token2_type
				)
			},
			Self::Token(id) => write!(f, "Token({})", id),
		}
	}
}

pub struct AssetConversion;

fn to_look_up_error(_: &'static str) -> LookupError {
	LookupError
}

impl StaticLookup for AssetConversion {
	type Source = CurrencyId;
	type Target = StellarAsset;

	fn lookup(
		currency_id: <Self as StaticLookup>::Source,
	) -> Result<<Self as StaticLookup>::Target, LookupError> {
		let asset_conversion_result: Result<StellarAsset, &str> = currency_id.try_into();
		asset_conversion_result.map_err(to_look_up_error)
	}

	fn unlookup(stellar_asset: <Self as StaticLookup>::Target) -> <Self as StaticLookup>::Source {
		CurrencyId::from(stellar_asset)
	}
}

pub struct StringCurrencyConversion;

impl Convert<(Vec<u8>, Vec<u8>), Result<CurrencyId, ()>> for StringCurrencyConversion {
	fn convert(a: (Vec<u8>, Vec<u8>)) -> Result<CurrencyId, ()> {
		let public_key = PublicKey::from_encoding(a.1).map_err(|_| ())?;
		let asset_code = from_utf8(a.0.as_slice()).map_err(|_| ())?;
		(asset_code, public_key.into_binary()).try_into().map_err(|_| ())
	}
}

// This struct can be used to convert from a 'Spacewalk' balance to a 'Stellar' balance.
// It converts the native balance of the chain to the stroop representation of the asset on Stellar.
pub struct BalanceConversion;

const CHAIN_DECIMALS: u32 = 12;
const STELLAR_DECIMALS: u32 = 7;
// We derive the conversion rate from the number of decimals of the chain and the number of decimals
// of the asset on Stellar.
const DECIMALS_CONVERSION_RATE: u128 = 10u128.pow(CHAIN_DECIMALS - STELLAR_DECIMALS);

// The type of stroop amounts is i64
// see [here](https://github.com/pendulum-chain/substrate-stellar-sdk/blob/f659041c6643f80f4e1f6e9e35268dba3ae2d313/src/amount.rs#L7)
pub type StellarStroops = i64;

pub fn stellar_stroops_to_u128(stellar_stroops: StellarStroops) -> u128 {
	let value = u128::try_from(stellar_stroops).unwrap_or(0);
	value.saturating_mul(DECIMALS_CONVERSION_RATE)
}

impl StaticLookup for BalanceConversion {
	type Source = u128;
	type Target = StellarStroops;

	fn lookup(pendulum_balance: Self::Source) -> Result<Self::Target, LookupError> {
		let stroops_u128: Self::Source = pendulum_balance / DECIMALS_CONVERSION_RATE;

		if stroops_u128 > Self::Target::MAX as Self::Source {
			Err(LookupError)
		} else {
			Ok(stroops_u128 as Self::Target)
		}
	}

	fn unlookup(stellar_stroops: Self::Target) -> Self::Source {
		stellar_stroops_to_u128(stellar_stroops)
	}
}

pub struct StellarCompatibility;

pub trait AmountCompatibility {
	type UnsignedFixedPoint: FixedPointNumber;

	fn is_compatible_with_target(
		source_amount: <<Self as AmountCompatibility>::UnsignedFixedPoint as FixedPointNumber>::Inner,
	) -> bool;

	#[allow(clippy::result_unit_err)]
	fn round_to_compatible_with_target(
		source_amount: <<Self as AmountCompatibility>::UnsignedFixedPoint as FixedPointNumber>::Inner,
	) -> Result<<<Self as AmountCompatibility>::UnsignedFixedPoint as FixedPointNumber>::Inner, ()>;
}

impl AmountCompatibility for StellarCompatibility {
	// We operate on the inner value of the FixedU128 type.
	type UnsignedFixedPoint = FixedU128;

	/// For Stellar we define an spacewalk-chain amount to be compatible with a Stellar amount if
	/// the amount has 5 trailing 0s. Because in this case the on-chain amounts can be truncated in
	/// the BalanceConversion functions without any loss of precision.
	fn is_compatible_with_target(source_amount: <FixedU128 as FixedPointNumber>::Inner) -> bool {
		source_amount % DECIMALS_CONVERSION_RATE == 0
	}

	/// We round the amount down to the nearest compatible amount, that is, we round the amount such
	/// that it has 5 trailing 0s. The result is either compatible with the target or an error is
	/// returned.
	fn round_to_compatible_with_target(
		source_amount: <FixedU128 as FixedPointNumber>::Inner,
	) -> Result<<FixedU128 as FixedPointNumber>::Inner, ()> {
		let conversion_rate = UnsignedFixedPoint::from_inner(DECIMALS_CONVERSION_RATE);
		let fixed_amount = UnsignedFixedPoint::from_inner(source_amount);

		let rounded_amount_fixed = fixed_amount
			// We first divide by the conversion rate to make it have the number of decimals we want
			// to round to (since we can only round decimals but are dealing with an integer).
			.checked_div(&conversion_rate)
			.ok_or(())?
			// Then we round
			.round()
			// Then we scale it back up to the correct number of decimals
			.checked_mul(&conversion_rate)
			.ok_or(())?;

		let rounded_amount = rounded_amount_fixed.into_inner();
		// This should always be true, but we check it just in case.
		if Self::is_compatible_with_target(rounded_amount) {
			Ok(rounded_amount)
		} else {
			Err(())
		}
	}
}

pub struct AddressConversion;

impl StaticLookup for AddressConversion {
	type Source = AccountId32;
	type Target = stellar::PublicKey;

	fn lookup(key: Self::Source) -> Result<Self::Target, LookupError> {
		// We just assume (!) an Ed25519 key has been passed to us
		Ok(stellar::PublicKey::from_binary(key.into()) as stellar::PublicKey)
	}

	fn unlookup(stellar_addr: stellar::PublicKey) -> Self::Source {
		MultiSigner::Ed25519(ed25519::Public::from_raw(*stellar_addr.as_binary())).into_account()
	}
}

pub trait TransactionEnvelopeExt {
	fn get_payment_amount_for_asset_to(&self, to: StellarPublicKeyRaw, asset: StellarAsset)
		-> u128;

	fn sequence_number(&self) -> Option<SequenceNumber>;

	fn get_transaction(&self) -> Option<Transaction>;
}

impl TransactionEnvelopeExt for TransactionEnvelope {
	/// Returns the amount of the given asset that is being sent to the given address.
	/// Only considers payment and claimable balance operations.
	fn get_payment_amount_for_asset_to(
		&self,
		to: StellarPublicKeyRaw,
		asset: StellarAsset,
	) -> u128 {
		let recipient_account_muxed = MuxedAccount::KeyTypeEd25519(to);
		let recipient_account_pk = PublicKey::PublicKeyTypeEd25519(to);

		let mut transferred_amount: StellarStroops = 0;

		let tx_operations: Vec<Operation> = match self {
			TransactionEnvelope::EnvelopeTypeTxV0(env) => env.tx.operations.get_vec().clone(),
			TransactionEnvelope::EnvelopeTypeTx(env) => env.tx.operations.get_vec().clone(),
			TransactionEnvelope::EnvelopeTypeTxFeeBump(_) => Vec::new(),
			TransactionEnvelope::Default(_) => Vec::new(),
		};

		if tx_operations.is_empty() {
			return BalanceConversion::unlookup(transferred_amount);
		}

		transferred_amount = tx_operations.iter().fold(0i64, |acc, x| {
			match &x.body {
				OperationBody::Payment(payment) => {
					if payment.destination.eq(&recipient_account_muxed) && payment.asset == asset {
						acc.saturating_add(payment.amount)
					} else {
						acc
					}
				},
				OperationBody::CreateClaimableBalance(payment) => {
					// for security reasons, we only count operations that have the
					// recipient as a single claimer and unconditional claim predicate
					if payment.claimants.len() == 1 {
						let Claimant::ClaimantTypeV0(claimant) = &payment.claimants.get_vec()[0];

						if claimant.destination.eq(&recipient_account_pk) &&
							payment.asset == asset && claimant.predicate ==
							ClaimPredicate::ClaimPredicateUnconditional
						{
							acc.saturating_add(payment.amount)
						} else {
							acc
						}
					} else {
						acc
					}
				},
				_ => acc, // ignore other operations
			}
		});

		// `transferred_amount` is in stroops, so we need to convert it
		BalanceConversion::unlookup(transferred_amount)
	}

	fn sequence_number(&self) -> Option<SequenceNumber> {
		match self {
			TransactionEnvelope::EnvelopeTypeTxV0(env) => Some(env.tx.seq_num),
			TransactionEnvelope::EnvelopeTypeTx(env) => Some(env.tx.seq_num),
			TransactionEnvelope::EnvelopeTypeTxFeeBump(_) | TransactionEnvelope::Default(_) => None,
		}
	}

	fn get_transaction(&self) -> Option<Transaction> {
		match self {
			TransactionEnvelope::EnvelopeTypeTxV0(transaction) =>
				Some(transaction.tx.clone().into()),
			TransactionEnvelope::EnvelopeTypeTx(transaction) => Some(transaction.tx.clone()),
			_ => None,
		}
	}
}
