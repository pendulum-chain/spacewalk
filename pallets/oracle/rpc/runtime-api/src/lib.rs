//! Runtime API definition for the Oracle.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode};
use frame_support::dispatch::DispatchError;
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

#[derive(Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
/// a wrapper around a balance, used in RPC to workaround a bug where using u128
/// in runtime-apis fails. See <https://github.com/paritytech/substrate/issues/4641>
pub struct BalanceWrapper<T> {
	#[cfg_attr(feature = "std", serde(bound(serialize = "T: std::fmt::Display")))]
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
	#[cfg_attr(feature = "std", serde(bound(deserialize = "T: std::str::FromStr")))]
	#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
	pub amount: T,
}

#[cfg(feature = "std")]
fn serialize_as_string<S: Serializer, T: std::fmt::Display>(
	t: &T,
	serializer: S,
) -> Result<S::Ok, S::Error> {
	serializer.serialize_str(&t.to_string())
}

// Adapted from https://www.reddit.com/r/rust/comments/fcz4yb/how_do_you_deserialize_strings_integers_to_float/
#[cfg(feature = "std")]
fn deserialize_from_string<'de, D: Deserializer<'de>, T: std::str::FromStr>(
	deserializer: D,
) -> Result<T, D::Error> {
	Ok(match Value::deserialize(deserializer)? {
		Value::String(s) => s.parse().map_err(|_| serde::de::Error::custom("Parse from string failed"))?,
		Value::Number(num) => {
			num.to_string().parse().map_err(|_| serde::de::Error::custom("Parse from number failed"))?
		},
		_ => return Err(serde::de::Error::custom("Type must be string or integer."))
	})
}

sp_api::decl_runtime_apis! {
	pub trait OracleApi<Balance, CurrencyId> where
		Balance: Codec,
		CurrencyId: Codec,
	{
		fn currency_to_usd(
			amount: BalanceWrapper<Balance>,
			currency_id: CurrencyId,
		) -> Result<BalanceWrapper<Balance>, DispatchError>;

		fn usd_to_currency(
			amount: BalanceWrapper<Balance>,
			currency_id: CurrencyId,
		) -> Result<BalanceWrapper<Balance>, DispatchError>;
	}
}
