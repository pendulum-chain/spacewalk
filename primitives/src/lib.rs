#![cfg_attr(not(feature = "std"), no_std)]

use bstringify::bstringify;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::error::LookupError;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sp_core::ed25519;
pub use sp_core::H256;
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, Convert, IdentifyAccount, StaticLookup, Verify},
	AccountId32, FixedI128, FixedPointNumber, FixedU128, MultiSignature, MultiSigner, RuntimeDebug,
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

	#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, MaxEncodedLen)]
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
	#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, MaxEncodedLen)]
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

pub mod oracle {
	use super::*;

	#[derive(Encode, Decode, Clone, Eq, PartialEq, Debug, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub enum Key {
		ExchangeRate(CurrencyId),
		FeeEstimation,
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
	$vis:vis enum TokenSymbol {
        $($(#[$vmeta:meta])* $symbol:ident($name:expr, $deci:literal) = $val:literal,)*
    }) => {
		$(#[$meta])*
		$vis enum TokenSymbol {
			$($(#[$vmeta])* $symbol = $val,)*
		}

        $(pub const $symbol: TokenSymbol = TokenSymbol::$symbol;)*

        impl TryFrom<u8> for TokenSymbol {
			type Error = ();

			fn try_from(v: u8) -> Result<Self, Self::Error> {
				match v {
					$($val => Ok(TokenSymbol::$symbol),)*
					_ => Err(()),
				}
			}
		}

		impl Into<u8> for TokenSymbol {
			fn into(self) -> u8 {
				match self {
					$(TokenSymbol::$symbol => ($val),)*
				}
			}
		}

        impl TokenSymbol {
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
					$(TokenSymbol::$symbol => $deci,)*
				}
			}
		}

		impl CurrencyInfo for TokenSymbol {
			fn name(&self) -> &str {
				match self {
					$(TokenSymbol::$symbol => $name,)*
				}
			}
			fn symbol(&self) -> &str {
				match self {
					$(TokenSymbol::$symbol => stringify!($symbol),)*
				}
			}
			fn decimals(&self) -> u8 {
				self.decimals()
			}
		}

		impl TryFrom<Vec<u8>> for TokenSymbol {
			type Error = ();
			fn try_from(v: Vec<u8>) -> Result<TokenSymbol, ()> {
				match v.as_slice() {
					$(bstringify!($symbol) => Ok(TokenSymbol::$symbol),)*
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
	pub enum TokenSymbol {
		DOT("Polkadot", 10) = 0,
		KSM("Kusama", 10) = 1,
		PEN("Pendulum", 10) = 3,
		AMPE("Amplitude", 12) = 13,
	}
}

#[derive(
	Encode, Decode, Eq, Hash, PartialEq, Copy, Clone, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum CurrencyId {
	Token(TokenSymbol),
	ForeignAsset(ForeignAssetId),
	Native,
	StellarNative,
	AlphaNum4 { code: Bytes4, issuer: AssetIssuer },
	AlphaNum12 { code: Bytes12, issuer: AssetIssuer },
}

pub type ForeignAssetId = u32;

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
			Self::Token(_) => Err("Token not defined in the Stellar world."),
			Self::ForeignAsset(_) => Err("Foreign Asset not defined in the Stellar world."),
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
			Self::Token(token_symbol) => {
				write!(f, "{:?} ({:?})", token_symbol.name(), token_symbol.symbol())
			},
			Self::ForeignAsset(id) => {
				write!(f, "{:?}", id)
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

pub struct BalanceConversion;

impl StaticLookup for BalanceConversion {
	type Source = u128;
	type Target = i64;

	fn lookup(pendulum_balance: Self::Source) -> Result<Self::Target, LookupError> {
		let stroops128: u128 = pendulum_balance / 100000;

		if stroops128 > i64::MAX as u128 {
			Err(LookupError)
		} else {
			Ok(stroops128 as i64)
		}
	}

	fn unlookup(stellar_stroops: Self::Target) -> Self::Source {
		(stellar_stroops * 100000) as u128
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
