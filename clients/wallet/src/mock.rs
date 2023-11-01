use crate::{
	error::Error,
	horizon::HorizonClient,
	operations::{
		create_basic_spacewalk_stellar_transaction, create_payment_operation,
		redeem_request_tests::create_account_merge_operation, AppendExt,
	},
	StellarWallet, TransactionResponse,
};
use primitives::{
	stellar::{
		types::SequenceNumber, Asset as StellarAsset, PublicKey, SecretKey, TransactionEnvelope,
	},
	CurrencyId, StellarStroops,
};
use std::sync::Arc;
use tokio::sync::RwLock;

pub const DEFAULT_DEST_PUBLIC_KEY: &str =
	"GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";
pub const STELLAR_VAULT_SECRET_KEY: &str =
	"SCV7RZN5XYYMMVSWYCR4XUMB76FFMKKKNHP63UTZQKVM4STWSCIRLWFJ";
pub const IS_PUBLIC_NETWORK: bool = false;

pub const DEFAULT_STROOP_FEE_PER_OPERATION: u32 = 100;

impl StellarWallet {
	pub async fn is_account_exist(&self) -> bool {
		self.client
			.get_account(self.public_key(), self.is_public_network())
			.await
			.is_ok()
	}

	/// merges the wallet's account with the specified destination.
	/// Exercise prudence when using this method, as it automatically removes the source account
	/// once operation is successful.
	pub async fn merge_account(
		&mut self,
		destination_address: PublicKey,
	) -> Result<TransactionResponse, Error> {
		let account_merge_op =
			create_account_merge_operation(destination_address, self.public_key())?;

		self.send_to_address([9u8; 32], DEFAULT_STROOP_FEE_PER_OPERATION, vec![account_merge_op])
			.await
	}

	pub fn create_payment_envelope(
		&self,
		destination_address: PublicKey,
		asset: StellarAsset,
		stroop_amount: StellarStroops,
		request_id: [u8; 32],
		stroop_fee_per_operation: u32,
		next_sequence_number: SequenceNumber,
	) -> Result<TransactionEnvelope, Error> {
		let public_key = self.public_key();
		// create payment operation
		let payment_op = create_payment_operation(
			destination_address,
			asset,
			stroop_amount,
			public_key.clone(),
		)?;

		self.create_envelope(
			request_id,
			stroop_fee_per_operation,
			next_sequence_number,
			vec![payment_op],
		)
	}

	pub fn create_payment_envelope_no_signature(
		&self,
		destination_address: PublicKey,
		asset: StellarAsset,
		stroop_amount: StellarStroops,
		request_id: [u8; 32],
		stroop_fee_per_operation: u32,
		next_sequence_number: SequenceNumber,
	) -> Result<TransactionEnvelope, Error> {
		let public_key = self.public_key();
		// create payment operation
		let payment_op = create_payment_operation(
			destination_address,
			asset,
			stroop_amount,
			public_key.clone(),
		)?;

		// create the transaction
		let mut transaction = create_basic_spacewalk_stellar_transaction(
			request_id,
			stroop_fee_per_operation,
			public_key,
			next_sequence_number,
		)?;

		transaction.append(payment_op)?;

		Ok(transaction.into_transaction_envelope())
	}
}

pub fn wallet_with_storage(storage: &str) -> Result<Arc<RwLock<StellarWallet>>, Error> {
	wallet_with_secret_key_for_storage(storage, STELLAR_VAULT_SECRET_KEY)
}

pub fn wallet_with_secret_key_for_storage(
	storage: &str,
	secret_key: &str,
) -> Result<Arc<RwLock<StellarWallet>>, Error> {
	Ok(Arc::new(RwLock::new(StellarWallet::from_secret_encoded_with_cache(
		secret_key,
		IS_PUBLIC_NETWORK,
		storage.to_string(),
	)?)))
}

pub fn default_destination() -> PublicKey {
	public_key_from_encoding(DEFAULT_DEST_PUBLIC_KEY)
}

pub const USDC_ISSUER: &str = "GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC";
pub fn default_usdc_asset() -> StellarAsset {
	let asset = CurrencyId::try_from(("USDC", USDC_ISSUER)).expect("should convert ok");
	asset.try_into().expect("should convert to Asset")
}

pub fn public_key_from_encoding<T: AsRef<[u8]>>(encoded_key: T) -> PublicKey {
	PublicKey::from_encoding(encoded_key).expect("should return a public key")
}

pub fn secret_key_from_encoding<T: AsRef<[u8]>>(encoded_key: T) -> SecretKey {
	SecretKey::from_encoding(encoded_key).expect("should return a secret key")
}
