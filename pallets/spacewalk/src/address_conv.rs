use frame_support::error::LookupError;
use sp_core::ed25519;
use sp_runtime::{
	traits::{IdentifyAccount, StaticLookup},
	AccountId32, MultiSigner,
};
use substrate_stellar_sdk as stellar;

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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;


	#[test]
	fn test_addr_conversion() {

		let account_id = AccountId32::from_str("5G9VdMwXvzza9pS8qE8ZHJk3CheHW9uucBn9ngW4C1gmmzpv").unwrap();

		let lookup_pk = AddressConversion::lookup(account_id.clone());

		assert!(lookup_pk.is_ok());

		let lookup_pk = lookup_pk.unwrap();

		let lookup_acc = AddressConversion::unlookup(lookup_pk);

		assert_eq!(lookup_acc, account_id);
	}
}