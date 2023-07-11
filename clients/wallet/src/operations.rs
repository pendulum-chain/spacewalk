use crate::{error::Error, horizon::HorizonClient};
use async_trait::async_trait;
use primitives::{derive_shortened_request_id, stellar_stroops_to_u128, StellarStroops};
use substrate_stellar_sdk::{
	compound_types::LimitedString,
	types::{Preconditions, SequenceNumber},
	Asset, ClaimPredicate, Claimant, Memo, Operation, PublicKey, StellarSdkError, StroopAmount,
	Transaction,
};

pub mod redeem {
	use super::*;

	pub async fn is_claimable_balance_op_required(
		destination_address: PublicKey,
		is_public_network: bool,
		to_be_redeemed_asset: Asset,
		to_be_redeemed_amount: primitives::StellarStroops,
	) -> Result<Operation, Vec<Operation>> {
		let horizon_client = reqwest::Client::new();

		horizon_client
			.claimable_balance_ops_required(
				destination_address,
				is_public_network,
				to_be_redeemed_asset,
				to_be_redeemed_amount,
			)
			.await
	}

	fn create_account_ops_required(
		destination_address: PublicKey,
		to_be_redeemed_asset: Asset,
		to_be_redeemed_amount: StellarStroops,
	) -> Option<Operation> {
		// create an account for the destination ONLY if redeeming >= 1 XLM.
		let to_be_redeemed_amount_u128 = stellar_stroops_to_u128(to_be_redeemed_amount);

		if to_be_redeemed_asset == Asset::AssetTypeNative &&
			to_be_redeemed_amount_u128 >= primitives::CurrencyId::StellarNative.one()
		{
			match create_account_operation(destination_address.clone(), to_be_redeemed_amount) {
				Ok(op) => return Some(op),
				Err(e) => {
					tracing::warn!("Failed to create CreateAccount operation: {e:?}");
				},
			}
		}
		None
	}

	#[async_trait]
	pub trait RedeemOps: HorizonClient {
		async fn claimable_balance_ops_required(
			&self,
			destination_address: PublicKey,
			is_public_network: bool,
			to_be_redeemed_asset: Asset,
			to_be_redeemed_amount: StellarStroops,
		) -> Result<Operation, Vec<Operation>> {
			// convert pubkey to string
			let dest_addr_encoded = destination_address.to_encoding();
			let dest_addr_str = std::str::from_utf8(&dest_addr_encoded).unwrap();

			// check if account exists
			match self.get_account(dest_addr_str, is_public_network).await {
				Ok(account) if account.is_trustline_exist(&to_be_redeemed_asset) => {
					// normal operation
					return Err(vec![])
				},
				Err(Error::HorizonSubmissionError {
					title: _,
					status,
					reason: _,
					envelope_xdr: _,
				}) if status == 404 =>
					if let Some(op) = create_account_ops_required(
						destination_address.clone(),
						to_be_redeemed_asset.clone(),
						to_be_redeemed_amount,
					) {
						return Err(vec![op])
					},
				_ => {},
			};

			let claimant =
				Claimant::new(destination_address, ClaimPredicate::ClaimPredicateUnconditional)
					.unwrap();
			// create claimable balance operation
			let create_claim_bal_op = create_claimable_balance_operation(
				to_be_redeemed_asset,
				to_be_redeemed_amount,
				vec![claimant],
			)
			.unwrap();

			Ok(create_claim_bal_op)
		}
	}

	#[async_trait]
	impl RedeemOps for reqwest::Client {}

	#[cfg(test)]
	mod tests {
		use super::*;
		use primitives::CurrencyId;
		use substrate_stellar_sdk::{ClaimPredicate::ClaimPredicateUnconditional, SecretKey};

		const STELLAR_VAULT_SECRET_KEY: &str =
			"SCV7RZN5XYYMMVSWYCR4XUMB76FFMKKKNHP63UTZQKVM4STWSCIRLWFJ";

		const INACTIVE_STELLAR_SECRET_KEY: &str =
			"SAOZUYCGHAHAHUN75JDPAEH7M42N64RN3AATZYB4X2MTXB6V7WV7O2IO";

		const USDC_ISSUER: &str = "GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC";
		const IS_PUBLIC_NETWORK: bool = false;

		#[test]
		fn create_account_ops_required_test() {
			let secret_key = SecretKey::from_encoding(STELLAR_VAULT_SECRET_KEY)
				.expect("should return secret key");
			let pub_key = secret_key.get_public().clone();

			// asset is XLM and value is >= 1
			assert!(create_account_ops_required(
				pub_key.clone(),
				Asset::AssetTypeNative,
				10_000_000
			)
			.is_some());

			// asset is XLM but value < 1
			assert!(create_account_ops_required(pub_key.clone(), Asset::AssetTypeNative, 10_000)
				.is_none());

			// asset is NOT XLM
			let asset = CurrencyId::try_from(("USDC", USDC_ISSUER)).expect("should convert ok");
			let asset = asset.try_into().expect("should convert to Asset");

			assert!(create_account_ops_required(pub_key.clone(), asset, 10_000_000).is_none());
		}

		#[tokio::test]
		async fn claimable_balance_ops_required_test() {
			let destination_pub_key = PublicKey::from_encoding(
				"GBWYDOKJT5BEYJCCBJRHL54AMLRWDALR2NLEY7CKMPSOUKWVJTCFQFYI",
			)
			.expect("should return secret key");

			// asset is NOT XLM AND has trustline, so it's normal op
			let existing_asset =
				CurrencyId::try_from(("USDC", USDC_ISSUER)).expect("should convert ok");
			let existing_asset = existing_asset.try_into().expect("should convert to Asset");

			assert_eq!(
				is_claimable_balance_ops_required(
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					existing_asset,
					10_000_000
				)
				.await,
				Err(vec![])
			);

			// asset is NOT XLM BUT has NO trustline, use claimable balance
			let unknown_asset = CurrencyId::try_from((
				"USDC",
				"GCXQXD3EDC7YILNAHRJLYR53MGQU4V5RXB7JHGPVS3HQHLBJDWCJHVIB",
			))
			.expect("should convert ok");
			let unknown_asset: Asset = unknown_asset.try_into().expect("should convert to Asset");
			let to_be_redeemed_amount = 10_000_000;
			assert_eq!(
				is_claimable_balance_ops_required(
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					unknown_asset.clone(),
					to_be_redeemed_amount
				)
				.await,
				Ok(create_claimable_balance_operation(
					unknown_asset.clone(),
					to_be_redeemed_amount,
					vec![Claimant::new(destination_pub_key, ClaimPredicateUnconditional)
						.expect("should return ok")]
				)
				.expect("should create claimablebalance ok"))
			);

			// account is not active, and asset is XLM with value >=1, use normal op
			let secret_key = SecretKey::from_encoding(INACTIVE_STELLAR_SECRET_KEY)
				.expect("should return secret key");
			let destination_pub_key = secret_key.get_public().clone();

			assert_eq!(
				is_claimable_balance_ops_required(
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					Asset::AssetTypeNative,
					to_be_redeemed_amount
				)
				.await,
				Err(vec![create_account_operation(
					destination_pub_key.clone(),
					to_be_redeemed_amount
				)
				.expect("should create createaccount op ok")])
			);

			// account is not active, and asset is XLM BUT value is < 1, use claimablebalance
			assert_eq!(
				is_claimable_balance_ops_required(
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					Asset::AssetTypeNative,
					to_be_redeemed_amount - 1000
				)
				.await,
				Ok(create_claimable_balance_operation(
					Asset::AssetTypeNative,
					to_be_redeemed_amount - 1000,
					vec![Claimant::new(destination_pub_key.clone(), ClaimPredicateUnconditional)
						.expect("should return ok")]
				)
				.expect("should create claimablebalance ok"))
			);

			// account is not active, and asset is NOT XLM, use claimablebalance
			assert_eq!(
				is_claimable_balance_ops_required(
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					unknown_asset.clone(),
					to_be_redeemed_amount
				)
				.await,
				Ok(create_claimable_balance_operation(
					unknown_asset,
					to_be_redeemed_amount,
					vec![Claimant::new(destination_pub_key.clone(), ClaimPredicateUnconditional)
						.expect("should return ok")]
				)
				.expect("should create claimablebalance ok"))
			);
		}
	}
}

fn change_error_type<T>(item: Result<T, StellarSdkError>) -> Result<T, Error> {
	item.map_err(|e| Error::BuildTransactionError(format!("Failed to create operation: {e:?}")))
}

pub fn create_claimable_balance_operation(
	asset: Asset,
	stroop_amount: StellarStroops,
	claimants: Vec<Claimant>,
) -> Result<Operation, Error> {
	let amount = StroopAmount(stroop_amount);
	change_error_type(Operation::new_create_claimable_balance(asset, amount, claimants))
}

pub fn create_account_operation(
	destination_address: PublicKey,
	starting_stroop_amount: StellarStroops,
) -> Result<Operation, Error> {
	let amount = StroopAmount(starting_stroop_amount);
	change_error_type(Operation::new_create_account(destination_address, amount))
}

pub fn create_payment_operation(
	destination_address: PublicKey,
	asset: Asset,
	stroop_amount: StellarStroops,
	public_key: PublicKey,
) -> Result<Operation, Error> {
	let stroop_amount = StroopAmount(stroop_amount);

	change_error_type(Operation::new_payment(destination_address, asset, stroop_amount))?
		.set_source_account(public_key)
		.map_err(|e| Error::BuildTransactionError(format!("Setting source account failed: {e:?}")))
}

pub fn create_basic_transaction(
	request_id: [u8; 32],
	stroop_fee_per_operation: u32,
	public_key: PublicKey,
	next_sequence_number: SequenceNumber,
) -> Result<Transaction, Error> {
	let memo_text = Memo::MemoText(
		LimitedString::new(derive_shortened_request_id(&request_id))
			.map_err(|_| Error::BuildTransactionError("Invalid hash".to_string()))?,
	);

	let transaction = change_error_type(Transaction::new(
		public_key.clone(),
		next_sequence_number,
		Some(stroop_fee_per_operation),
		Preconditions::PrecondNone,
		Some(memo_text),
	))?;

	Ok(transaction)
}

pub trait AppendExt<T> {
	fn append_multiple(&mut self, items: Vec<T>) -> Result<(), Error>;
	fn append(&mut self, item: T) -> Result<(), Error>;
}

impl AppendExt<Operation> for Transaction {
	fn append_multiple(&mut self, items: Vec<Operation>) -> Result<(), Error> {
		for operation in items {
			self.append(operation)?;
		}
		Ok(())
	}

	fn append(&mut self, item: Operation) -> Result<(), Error> {
		self.append_operation(item).map_err(|e| {
			Error::BuildTransactionError(format!("Appending payment operation failed: {e:?}"))
		})?;
		Ok(())
	}
}
