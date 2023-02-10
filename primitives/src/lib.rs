#![cfg_attr(not(feature = "std"), no_std)]
#![allow(non_upper_case_globals)]

use bstringify::bstringify;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::error::LookupError;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
pub use sp_core::H256;
use sp_core::{crypto::AccountId32, ed25519};
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, Convert, IdentifyAccount, StaticLookup, Verify},
	FixedI128, FixedPointNumber, FixedU128, MultiSignature, MultiSigner, RuntimeDebug,
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
	Asset, PublicKey,
};
pub use substrate_stellar_sdk as stellar;
use substrate_stellar_sdk::{
	types::OperationBody, ClaimPredicate, Claimant, MuxedAccount, Operation, TransactionEnvelope,
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

#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, PartialOrd, Ord, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, std::hash::Hash))]
pub struct VaultCurrencyPair<CurrencyId: Copy> {
	pub collateral: CurrencyId,
	pub wrapped: CurrencyId,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, PartialOrd, Ord, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, std::hash::Hash))]
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

	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub enum IssueRequestStatus {
		/// opened, but not yet executed or cancelled
		Pending,
		/// payment was received
		Completed,
		/// payment was not received, vault may receive griefing collateral
		Cancelled,
	}

	impl Default for IssueRequestStatus {
		fn default() -> Self {
			IssueRequestStatus::Pending
		}
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

	#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub enum RedeemRequestStatus {
		/// opened, but not yet executed or cancelled
		Pending,
		/// successfully executed with a valid payment from the vault
		Completed,
		/// bool=true indicates that the vault minted tokens for the amount that the redeemer
		/// burned
		Reimbursed(bool),
		/// user received compensation, but is retrying the redeem with another vault
		Retried,
	}

	impl Default for RedeemRequestStatus {
		fn default() -> Self {
			RedeemRequestStatus::Pending
		}
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

	#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize, Eq))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub enum ReplaceRequestStatus {
		/// accepted, but not yet executed or cancelled
		Pending,
		/// successfully executed with a valid payment from the old vault
		Completed,
		/// payment was not received, new vault may receive griefing collateral
		Cancelled,
	}

	impl Default for ReplaceRequestStatus {
		fn default() -> Self {
			ReplaceRequestStatus::Pending
		}
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

	#[derive(Encode, Decode, Clone, Eq, PartialEq, Debug, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
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

pub trait CurrencyInfo {
	fn name(&self) -> &str;
	fn symbol(&self) -> &str;
	fn decimals(&self) -> u8;
}

macro_rules! create_currency_id {
    ($(#[$meta:meta])*
	$vis:vis enum ForeignCurrencyId {
        $($(#[$vmeta:meta])* $symbol:ident($name:expr, $deci:literal) = $val:literal,)*
    }) => {
		$(#[$meta])*
		$vis enum ForeignCurrencyId {
			$($(#[$vmeta])* $symbol = $val,)*
		}

        $(pub const $symbol: ForeignCurrencyId = ForeignCurrencyId::$symbol;)*

        impl TryFrom<u8> for ForeignCurrencyId {
			type Error = ();

			fn try_from(v: u8) -> Result<Self, Self::Error> {
				match v {
					$($val => Ok(ForeignCurrencyId::$symbol),)*
					_ => Err(()),
				}
			}
		}

		impl TryFrom<u64> for ForeignCurrencyId {
			type Error = ();

			fn try_from(v: u64) -> Result<Self, Self::Error> {
				match v {
					$($val => Ok(ForeignCurrencyId::$symbol),)*
					_ => Err(()),
				}
			}
		}

		impl Into<u8> for ForeignCurrencyId {
			fn into(self) -> u8 {
				match self {
					$(ForeignCurrencyId::$symbol => ($val),)*
				}
			}
		}

        impl ForeignCurrencyId {
			pub fn get_info() -> Vec<(&'static str, u32)> {
				vec![
					$((stringify!($symbol), $deci),)*
				]
			}

            pub const fn one(&self) -> Balance {
                10u128.pow(self.decimals() as u32)
            }

            const fn decimals(&self) -> u8 {
				match self {
					$(ForeignCurrencyId::$symbol => $deci,)*
				}
			}
		}

		impl CurrencyInfo for ForeignCurrencyId {
			fn name(&self) -> &str {
				match self {
					$(ForeignCurrencyId::$symbol => $name,)*
				}
			}
			fn symbol(&self) -> &str {
				match self {
					$(ForeignCurrencyId::$symbol => stringify!($symbol),)*
				}
			}
			fn decimals(&self) -> u8 {
				self.decimals()
			}
		}

		impl TryFrom<Vec<u8>> for ForeignCurrencyId {
			type Error = ();
			fn try_from(v: Vec<u8>) -> Result<ForeignCurrencyId, ()> {
				match v.as_slice() {
					$(bstringify!($symbol) => Ok(ForeignCurrencyId::$symbol),)*
					_ => Err(()),
				}
			}
		}
    }
}

create_currency_id! {
	#[derive(Encode, Decode, Eq, Hash, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	#[repr(u8)]
	pub enum ForeignCurrencyId {
		KSM("Kusama", 10) = 0,
		KAR("Karura",10) = 1,
		AUSD("Acala Karura",10) = 2,
		BNC("Bifrost",10)= 3,
		VsKSM("Kusama Bifrost",10) = 4,
		HKO("Heiko", 10) = 5,
		MOVR("Moonriver", 10) = 6,
		SDN("Shiden", 10) = 7,
		KINT("Kintsugi", 10) = 8,
		KBTC("Kintsugi BTC", 10) = 9,
		GENS("Genshiro", 10) = 10,
		XOR("Sora", 10) = 11,
		TEER("Integritee", 10) = 12,
		KILT("Kilt", 10) = 13,
		PHA("Phala", 10) = 14,
		ZTG("Zeitgeist", 10) = 15,
		USD("Statemine", 10) = 16,
		DOT("Polkadot", 10) = 17,
	}
}

#[derive(
	Encode, Decode, Eq, Hash, PartialEq, Copy, Clone, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum CurrencyId {
	XCM(ForeignCurrencyId),
	Native,
	StellarNative,
	AlphaNum4 { code: Bytes4, issuer: AssetIssuer },
	AlphaNum12 { code: Bytes12, issuer: AssetIssuer },
}

#[derive(scale_info::TypeInfo, Encode, Decode, Clone, Eq, PartialEq, Debug)]
pub struct CustomMetadata {
	pub fee_per_second: u128,
	pub coingecko_id: Vec<u8>,
}

pub type Bytes4 = [u8; 4];
pub type Bytes12 = [u8; 12];
pub type AssetIssuer = [u8; 32];

impl Default for CurrencyId {
	fn default() -> Self {
		CurrencyId::Native
	}
}

impl TryFrom<(&str, &str)> for CurrencyId {
	type Error = &'static str;

	fn try_from(value: (&str, &str)) -> Result<Self, Self::Error> {
		let slice = value.0;
		let issuer_encoded = value.1;
		let issuer_pk = stellar::PublicKey::from_encoding(issuer_encoded)
			.map_err(|_| "Invalid issuer encoding")?;
		let issuer: AssetIssuer = *issuer_pk.as_binary();
		if slice.len() <= 4 {
			let mut code: Bytes4 = [0; 4];
			code[..slice.len()].copy_from_slice(slice.as_bytes());
			Ok(CurrencyId::AlphaNum4 { code, issuer })
		} else if slice.len() > 4 && slice.len() <= 12 {
			let mut code: Bytes12 = [0; 12];
			code[..slice.len()].copy_from_slice(slice.as_bytes());
			Ok(CurrencyId::AlphaNum12 { code, issuer })
		} else {
			Err("More than 12 bytes not supported")
		}
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
			Ok(CurrencyId::AlphaNum4 { code, issuer })
		} else if slice.len() > 4 && slice.len() <= 12 {
			let mut code: Bytes12 = [0; 12];
			code[..slice.len()].copy_from_slice(slice.as_bytes());
			Ok(CurrencyId::AlphaNum12 { code, issuer })
		} else {
			Err("More than 12 bytes not supported")
		}
	}
}

impl From<stellar::Asset> for CurrencyId {
	fn from(asset: stellar::Asset) -> Self {
		match asset {
			stellar::Asset::Default(_) => CurrencyId::StellarNative,
			stellar::Asset::AssetTypeNative => CurrencyId::StellarNative,
			stellar::Asset::AssetTypeCreditAlphanum4(asset_alpha_num4) => CurrencyId::AlphaNum4 {
				code: asset_alpha_num4.asset_code,
				issuer: asset_alpha_num4.issuer.into_binary(),
			},
			stellar::Asset::AssetTypeCreditAlphanum12(asset_alpha_num12) =>
				CurrencyId::AlphaNum12 {
					code: asset_alpha_num12.asset_code,
					issuer: asset_alpha_num12.issuer.into_binary(),
				},
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
			Self::AlphaNum4 { code, issuer } =>
				Ok(stellar::Asset::AssetTypeCreditAlphanum4(AlphaNum4 {
					asset_code: code,
					issuer: PublicKey::PublicKeyTypeEd25519(issuer),
				})),
			Self::AlphaNum12 { code, issuer } =>
				Ok(stellar::Asset::AssetTypeCreditAlphanum12(AlphaNum12 {
					asset_code: code,
					issuer: PublicKey::PublicKeyTypeEd25519(issuer),
				})),
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
			Self::StellarNative => write!(f, "XLM"),
			Self::AlphaNum4 { code, issuer } => {
				write!(
					f,
					"{{ code: {}, issuer: {} }}",
					str::from_utf8(code).unwrap(),
					str::from_utf8(
						stellar::PublicKey::from_binary(*issuer).to_encoding().as_slice()
					)
					.unwrap()
				)
			},
			Self::AlphaNum12 { code, issuer } => {
				write!(
					f,
					"{{ code: {}, issuer: {} }}",
					str::from_utf8(code).unwrap(),
					str::from_utf8(
						stellar::PublicKey::from_binary(*issuer).to_encoding().as_slice()
					)
					.unwrap()
				)
			},
		}
	}
}

pub struct AssetConversion;

fn to_look_up_error(_: &'static str) -> LookupError {
	LookupError
}

impl StaticLookup for AssetConversion {
	type Source = CurrencyId;
	type Target = Asset;

	fn lookup(
		currency_id: <Self as StaticLookup>::Source,
	) -> Result<<Self as StaticLookup>::Target, LookupError> {
		let asset_conversion_result: Result<Asset, &str> = currency_id.try_into();
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

// We set the conversion rate to 1:1 for now.
const CONVERSION_RATE: u128 = 1;

impl StaticLookup for BalanceConversion {
	type Source = u128;
	// The type of stroop amounts is i64
	// see [here](https://github.com/pendulum-chain/substrate-stellar-sdk/blob/f659041c6643f80f4e1f6e9e35268dba3ae2d313/src/amount.rs#L7)
	type Target = i64;

	fn lookup(pendulum_balance: Self::Source) -> Result<Self::Target, LookupError> {
		let stroops128: u128 = pendulum_balance / CONVERSION_RATE;

		if stroops128 > i64::MAX as u128 {
			Err(LookupError)
		} else {
			Ok(stroops128 as i64)
		}
	}

	fn unlookup(stellar_stroops: Self::Target) -> Self::Source {
		(stellar_stroops * CONVERSION_RATE as i64) as u128
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

/// Error type for key decoding errors
#[derive(Debug)]
pub enum AddressConversionError {
	//     UnexpectedKeyType
}

pub trait TransactionEnvelopeExt {
	fn get_payment_amount_for_asset_to(&self, to: StellarPublicKeyRaw, asset: Asset) -> u128;
}

impl TransactionEnvelopeExt for TransactionEnvelope {
	/// Returns the amount of the given asset that is being sent to the given address.
	/// Only considers payment and claimable balance operations.
	fn get_payment_amount_for_asset_to(&self, to: StellarPublicKeyRaw, asset: Asset) -> u128 {
		let recipient_account_muxed = MuxedAccount::KeyTypeEd25519(to);
		let recipient_account_pk = PublicKey::PublicKeyTypeEd25519(to);

		let tx_operations: Vec<Operation> = match self {
			TransactionEnvelope::EnvelopeTypeTxV0(env) => env.tx.operations.get_vec().clone(),
			TransactionEnvelope::EnvelopeTypeTx(env) => env.tx.operations.get_vec().clone(),
			TransactionEnvelope::EnvelopeTypeTxFeeBump(_) => Vec::new(),
			TransactionEnvelope::Default(_) => Vec::new(),
		};

		let mut transferred_amount: i64 = 0;
		for x in tx_operations {
			match x.body {
				OperationBody::Payment(payment) => {
					if payment.destination.eq(&recipient_account_muxed) && payment.asset == asset {
						transferred_amount = transferred_amount.saturating_add(payment.amount);
					}
				},
				OperationBody::CreateClaimableBalance(payment) => {
					// for security reasons, we only count operations that have the
					// recipient as a single claimer and unconditional claim predicate
					if payment.claimants.len() == 1 {
						let Claimant::ClaimantTypeV0(claimant) =
							payment.claimants.get_vec()[0].clone();

						if claimant.destination.eq(&recipient_account_pk) &&
							payment.asset == asset && claimant.predicate ==
							ClaimPredicate::ClaimPredicateUnconditional
						{
							transferred_amount = transferred_amount.saturating_add(payment.amount);
						}
					}
				},
				_ => {
					// ignore other operations
				},
			}
		}

		// `transferred_amount` is in stroops, so we need to convert it
		BalanceConversion::unlookup(transferred_amount)
	}
}

use scale_info::prelude::string::String;
use sp_std::vec;

pub struct DiaOracleKeyConvertor;
impl Convert<oracle::Key, Option<(Vec<u8>, Vec<u8>)>> for DiaOracleKeyConvertor {
	fn convert(spacwalk_oracle_key: oracle::Key) -> Option<(Vec<u8>, Vec<u8>)> {
		match spacwalk_oracle_key {
			oracle::Key::ExchangeRate(currency_id) => match currency_id {
				CurrencyId::XCM(token_symbol) => match token_symbol {
					ForeignCurrencyId::DOT => return Some((vec![0u8], vec![1u8])),
					ForeignCurrencyId::KSM => return Some((vec![0u8], vec![3u8])),
					_ => None,
				},
				CurrencyId::Native => Some((vec![2u8], vec![])),
				CurrencyId::StellarNative => Some((vec![3u8], vec![])),
				CurrencyId::AlphaNum4 { code, .. } => Some((vec![4u8], code.to_vec())),
				CurrencyId::AlphaNum12 { code, .. } => Some((vec![5u8], code.to_vec())),
			},
		}
	}
}

impl Convert<(Vec<u8>, Vec<u8>), Option<oracle::Key>> for DiaOracleKeyConvertor {
	fn convert(dia_oracle_key: (Vec<u8>, Vec<u8>)) -> Option<oracle::Key> {
		let (blockchain, symbol) = dia_oracle_key;
		match blockchain[0] {
			0u8 => match symbol[0] {
				1 =>
					return Some(oracle::Key::ExchangeRate(CurrencyId::XCM(ForeignCurrencyId::DOT))),
				3 =>
					return Some(oracle::Key::ExchangeRate(CurrencyId::XCM(ForeignCurrencyId::KSM))),
				_ => return None,
			},
			2u8 => Some(oracle::Key::ExchangeRate(CurrencyId::Native)),
			3u8 => Some(oracle::Key::ExchangeRate(CurrencyId::StellarNative)),
			4u8 => {
				let vector = symbol;
				let code = [vector[0], vector[1], vector[2], vector[3]];
				Some(oracle::Key::ExchangeRate(CurrencyId::AlphaNum4 { code, issuer: [0u8; 32] }))
			},
			5u8 => {
				let vector = symbol;
				let code = [
					vector[0], vector[1], vector[2], vector[3], vector[4], vector[5], vector[6],
					vector[7], vector[8], vector[9], vector[10], vector[11],
				];
				Some(oracle::Key::ExchangeRate(CurrencyId::AlphaNum12 { code, issuer: [0u8; 32] }))
			},
			_ => None,
		}
	}
}

// impl TryFrom<u64> for ForeignCurrencyId {
// 	type Error = ();
// 	fn try_from(num: u64) -> Result<Self, Self::Error> {
// 		match num {
// 			0 => Ok(ForeignCurrencyId::KSM),
// 			1 => Ok(ForeignCurrencyId::KAR),
// 			2 => Ok(ForeignCurrencyId::AUSD),
// 			3 => Ok(ForeignCurrencyId::BNC),
// 			4 => Ok(ForeignCurrencyId::VsKSM),
// 			5 => Ok(ForeignCurrencyId::HKO),
// 			6 => Ok(ForeignCurrencyId::MOVR),
// 			7 => Ok(ForeignCurrencyId::SDN),
// 			8 => Ok(ForeignCurrencyId::KINT),
// 			9 => Ok(ForeignCurrencyId::KBTC),
// 			10 => Ok(ForeignCurrencyId::GENS),
// 			11 => Ok(ForeignCurrencyId::XOR),
// 			12 => Ok(ForeignCurrencyId::TEER),
// 			13 => Ok(ForeignCurrencyId::KILT),
// 			14 => Ok(ForeignCurrencyId::PHA),
// 			15 => Ok(ForeignCurrencyId::ZTG),
// 			16 => Ok(ForeignCurrencyId::USD),
// 			_ => Err(()),
// 		}
// 	}
// }
