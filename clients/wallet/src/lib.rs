pub use horizon::{
	listen_for_new_transactions,
	responses::{HorizonBalance, TransactionResponse},
};
pub use stellar_wallet::StellarWallet;
pub use task::*;

mod cache;
pub mod error;
mod horizon;
pub mod operations;
mod stellar_wallet;
mod task;
pub mod types;

pub use types::{LedgerTxEnvMap, Slot};

pub type TransactionsResponseIter = horizon::responses::TransactionsResponseIter<reqwest::Client>;

#[cfg(test)]
pub mod test_helper {
	use primitives::{
		stellar::{Asset, PublicKey, SecretKey},
		CurrencyId,
	};

	pub const USDC_ISSUER: &str = "GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC";
	pub fn default_usdc_asset() -> Asset {
		let asset = CurrencyId::try_from(("USDC", USDC_ISSUER)).expect("should convert ok");
		asset.try_into().expect("should convert to Asset")
	}

	pub fn public_key_from_encoding<T: AsRef<[u8]>>(encoded_key: T) -> PublicKey {
		PublicKey::from_encoding(encoded_key).expect("should return a public key")
	}

	pub fn secret_key_from_encoding<T: AsRef<[u8]>>(encoded_key: T) -> SecretKey {
		SecretKey::from_encoding(encoded_key).expect("should return a secret key")
	}
}
