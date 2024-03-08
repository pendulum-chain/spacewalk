use serde::{Deserialize, Deserializer};
use std::str::FromStr;

pub fn de_str_to_bytes<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	Ok(s.as_bytes().to_vec())
}

pub fn de_str_to_u128<'de, D>(de: D) -> Result<u128, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	u128::from_str(s).map_err(serde::de::Error::custom)
}

pub fn de_str_to_u64<'de, D>(de: D) -> Result<u64, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	u64::from_str(s).map_err(serde::de::Error::custom)
}

pub fn de_str_to_i64<'de, D>(de: D) -> Result<i64, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	i64::from_str(s).map_err(serde::de::Error::custom)
}

pub fn de_str_to_f64<'de, D>(de: D) -> Result<f64, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	f64::from_str(s).map_err(serde::de::Error::custom)
}

pub fn de_str_to_u32<'de, D>(de: D) -> Result<u32, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	u32::from_str(s).map_err(serde::de::Error::custom)
}

pub fn de_str_to_optional_bytes<'de, D>(de: D) -> Result<Option<Vec<u8>>, D::Error>
where
	D: Deserializer<'de>,
{
	Option::<&str>::deserialize(de).map(|opt_wrapped| opt_wrapped.map(|x| x.as_bytes().to_vec()))
}

pub fn de_string_to_optional_bytes<'de, D>(de: D) -> Result<Option<Vec<u8>>, D::Error>
where
	D: Deserializer<'de>,
{
	Option::<String>::deserialize(de).map(|opt_wrapped| opt_wrapped.map(|x| x.as_bytes().to_vec()))
}

pub fn de_string_to_optional_bytes<'de, D>(de: D) -> Result<Option<Vec<u8>>, D::Error>
where
	D: Deserializer<'de>,
{
	Option::<String>::deserialize(de).map(|opt_wrapped| opt_wrapped.map(|x| x.as_bytes().to_vec()))
}
