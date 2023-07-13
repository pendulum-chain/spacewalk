use crate::{error::Error, horizon::HorizonClient};
use async_trait::async_trait;
use primitives::{derive_shortened_request_id, stellar_stroops_to_u128, StellarStroops};
use substrate_stellar_sdk::{
	compound_types::LimitedString,
	types::{Preconditions, SequenceNumber},
	Asset, ClaimPredicate, Claimant, Memo, Operation, PublicKey, StellarSdkError, StroopAmount,
	Transaction,
};

#[async_trait]
pub trait RedeemOperationsExt: HorizonClient {
	async fn create_payment_op_for_redeem_request(
		&self,
		source_address: PublicKey,
		destination_address: PublicKey,
		is_public_network: bool,
		to_be_redeemed_asset: Asset,
		to_be_redeemed_amount: StellarStroops,
	) -> Result<Operation, Error> {
		// convert pubkey to string

		let claimable_balance_operation = || {
			create_unconditional_claimable_balance_operation(
				destination_address.clone(),
				to_be_redeemed_asset.clone(),
				to_be_redeemed_amount,
			)
		};

		// check if account exists
		match self.get_account(destination_address.clone(), is_public_network).await {
			// if account exists and trustline exists, use normal payment operation
			Ok(account) if account.is_trustline_exist(&to_be_redeemed_asset) =>
			// normal operation
				create_payment_operation(
					destination_address,
					to_be_redeemed_asset.clone(),
					to_be_redeemed_amount,
					source_address,
				),
			// if account exists and NO trustline, use claimable balance operation
			Ok(_) => claimable_balance_operation(),
			// if INactive account...
			Err(Error::HorizonSubmissionError { title: _, status, reason: _, envelope_xdr: _ })
				if status == 404 =>
			{
				let to_be_redeemed_amount_u128 = stellar_stroops_to_u128(to_be_redeemed_amount);

				// ... and redeeming amount >= 1 XLM, use create account operation
				if to_be_redeemed_asset == Asset::AssetTypeNative &&
					to_be_redeemed_amount_u128 >= primitives::CurrencyId::StellarNative.one()
				{
					create_account_operation(destination_address, to_be_redeemed_amount)
				}
				// else use claimable balance
				else {
					claimable_balance_operation()
				}
			},
			Err(e) => Err(e),
		}
	}
}

#[async_trait]
impl RedeemOperationsExt for reqwest::Client {}

fn change_error_type<T>(item: Result<T, StellarSdkError>) -> Result<T, Error> {
	item.map_err(|e| Error::BuildTransactionError(format!("Failed to create operation: {e:?}")))
}

fn create_unconditional_claimable_balance_operation(
	destination_address: PublicKey,
	to_be_redeemed_asset: Asset,
	to_be_redeemed_amount: StellarStroops,
) -> Result<Operation, Error> {
	let claimant =
		Claimant::new(destination_address.clone(), ClaimPredicate::ClaimPredicateUnconditional)
			.map_err(|e| {
				tracing::error!("cannot create claimant: {e:?}");

				Error::BuildTransactionError(
					format!("Failed to create ClaimableBalance operation for claimant {destination_address:?}")
				)
			})?;

	create_claimable_balance_operation(to_be_redeemed_asset, to_be_redeemed_amount, vec![claimant])
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

#[cfg(test)]
mod tests {
	use super::*;
	use primitives::CurrencyId;
	use substrate_stellar_sdk::{
		types::{
			CreateClaimableBalanceResult, OperationResult, OperationResultTr,
			TransactionResultResult,
		},
		SecretKey, XdrCodec,
	};

	const INACTIVE_STELLAR_SECRET_KEY: &str =
		"SAOZUYCGHAHAHUN75JDPAEH7M42N64RN3AATZYB4X2MTXB6V7WV7O2IO";

	const USDC_ISSUER: &str = "GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC";
	const IS_PUBLIC_NETWORK: bool = false;

	#[tokio::test]
	async fn create_payment_op_for_redeem_request() {
		let client = reqwest::Client::new();

		let source_pub_key =
			PublicKey::from_encoding("GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN")
				.expect("should return public key");

		let destination_pub_key =
			PublicKey::from_encoding("GBWYDOKJT5BEYJCCBJRHL54AMLRWDALR2NLEY7CKMPSOUKWVJTCFQFYI")
				.expect("should return public key");

		// existing account with XLM asset, use normal op
		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key.clone(),
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					Asset::AssetTypeNative,
					10_000_000
				)
				.await
				.expect("should not fail"),
			create_payment_operation(
				destination_pub_key.clone(),
				Asset::AssetTypeNative,
				10_000_000,
				source_pub_key.clone()
			)
			.expect("should not fail")
		);

		// existing account with non-XLM asset and has a trustline, use normal op
		let existing_asset =
			CurrencyId::try_from(("USDC", USDC_ISSUER)).expect("should convert ok");
		let existing_asset: Asset = existing_asset.try_into().expect("should convert to Asset");

		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key.clone(),
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					existing_asset.clone(),
					10_000_000
				)
				.await
				.expect("should not fail"),
			create_payment_operation(
				destination_pub_key.clone(),
				existing_asset.clone(),
				10_000_000,
				source_pub_key.clone()
			)
			.expect("should not fail")
		);

		// existing account with non-XLM asset BUT has NO trustline, use claimable balance
		let unknown_asset = CurrencyId::try_from((
			"USDC",
			"GCXQXD3EDC7YILNAHRJLYR53MGQU4V5RXB7JHGPVS3HQHLBJDWCJHVIB",
		))
		.expect("should convert ok");
		let unknown_asset: Asset = unknown_asset.try_into().expect("should convert to Asset");
		let to_be_redeemed_amount = 10_000_000;
		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key.clone(),
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					unknown_asset.clone(),
					to_be_redeemed_amount
				)
				.await
				.expect("should not fail"),
			create_unconditional_claimable_balance_operation(
				destination_pub_key.clone(),
				unknown_asset.clone(),
				to_be_redeemed_amount,
			)
			.expect("should not fail")
		);

		// INactive account and XLM asset of value >=1, use create account op
		let secret_key = SecretKey::from_encoding(INACTIVE_STELLAR_SECRET_KEY)
			.expect("should return secret key");
		let destination_pub_key = secret_key.get_public().clone();

		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key.clone(),
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					Asset::AssetTypeNative,
					to_be_redeemed_amount
				)
				.await
				.expect("should not fail"),
			create_account_operation(destination_pub_key.clone(), to_be_redeemed_amount)
				.expect("should not fail")
		);

		// INactive account but XLM asset of value < 1, use claimable balance
		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key.clone(),
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					Asset::AssetTypeNative,
					to_be_redeemed_amount - 1000
				)
				.await
				.expect("should not fail"),
			create_unconditional_claimable_balance_operation(
				destination_pub_key.clone(),
				Asset::AssetTypeNative,
				to_be_redeemed_amount - 1000,
			)
			.expect("should not fail")
		);

		// INactive account and asset is NOT XLM, use claimable balance
		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key,
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					unknown_asset.clone(),
					to_be_redeemed_amount
				)
				.await
				.expect("should not fail"),
			create_unconditional_claimable_balance_operation(
				destination_pub_key,
				unknown_asset,
				to_be_redeemed_amount,
			)
			.expect("should not fail")
		);
	}
}
