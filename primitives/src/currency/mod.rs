use frame_support::error::LookupError;
use sp_core::ed25519;
use sp_runtime::{
	scale_info::TypeInfo,
	traits::{Convert, IdentifyAccount, StaticLookup},
	AccountId32, MultiSigner,
};
use sp_std::{
	convert::{From, TryFrom, TryInto},
	fmt, str,
	str::from_utf8,
	vec::Vec,
};

use stellar::{
	types::{AlphaNum12, AlphaNum4},
	Asset, PublicKey,
};
use substrate_stellar_sdk as stellar;

use codec::{Decode, Encode, MaxEncodedLen};

pub type Bytes4 = [u8; 4];
pub type Bytes12 = [u8; 12];
pub type AssetIssuer = [u8; 32];

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Native,
	StellarNative,
	AlphaNum4 { code: Bytes4, issuer: AssetIssuer },
	AlphaNum12 { code: Bytes12, issuer: AssetIssuer },
}

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
			Self::Native => write!(f, "PEN"),
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

pub struct CurrencyConversion;

fn to_look_up_error(_: &'static str) -> LookupError {
	LookupError
}

impl StaticLookup for CurrencyConversion {
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
