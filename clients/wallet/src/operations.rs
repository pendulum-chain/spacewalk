use crate::{error::Error, horizon::HorizonClient};
use async_trait::async_trait;
use primitives::{
	derive_shortened_request_id,
	stellar::{
		compound_types::LimitedString,
		types::{Preconditions, SequenceNumber},
		Asset, ClaimPredicate, Claimant, Memo, Operation, PublicKey, StellarSdkError, StroopAmount,
		Transaction,
	},
	stellar_stroops_to_u128, StellarStroops,
};

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
		self.append_operation(item)
			.map_err_as_build_tx_error_with_text("Appending payment operation failed")
	}
}

/// Converts external error into wallet's `BuildTransactionError`.
trait BuildTransactionErrorExt<T> {
	fn map_err_as_build_tx_error(self) -> Result<T, Error>;

	fn map_err_as_build_tx_error_with_text(self, text: &str) -> Result<T, Error>;
}

impl<T> BuildTransactionErrorExt<T> for Result<T, StellarSdkError> {
	fn map_err_as_build_tx_error(self) -> Result<T, Error> {
		self.map_err(|e| {
			Error::BuildTransactionError(format!("Failed to build transaction: {e:?}"))
		})
	}

	fn map_err_as_build_tx_error_with_text(self, text: &str) -> Result<T, Error> {
		self.map_err(|e| Error::BuildTransactionError(format!("{text}: {e:?}")))
	}
}

#[async_trait]
pub trait RedeemOperationsExt: HorizonClient {
	/// Creates an appropriate payment operation for redeem requests.
	/// Possible operations are the ff:
	/// * `Payment` operation
	/// * `CreateClaimableBalance` operation
	/// * `CreateAccount` operation
	///
	/// # Arguments
	/// * `source_address` - the account executing the payment
	/// * `destination_address` - receiver of the payment
	/// * `is_public_network` - the network where the query should be executed
	/// * `to_be_redeemed_asset` - Stellar Asset type of the payment
	/// * `to_be_redeemed_amount` - Amount of the payment
	async fn create_payment_op_for_redeem_request(
		&self,
		source_address: PublicKey,
		destination_address: PublicKey,
		is_public_network: bool,
		to_be_redeemed_asset: Asset,
		to_be_redeemed_amount: StellarStroops,
	) -> Result<Operation, Error> {
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
			Err(Error::HorizonSubmissionError { status, .. })
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

fn create_unconditional_claimable_balance_operation(
	destination_address: PublicKey,
	to_be_redeemed_asset: Asset,
	to_be_redeemed_amount: StellarStroops,
) -> Result<Operation, Error> {
	let claimant = Claimant::new(destination_address, ClaimPredicate::ClaimPredicateUnconditional)
		.map_err_as_build_tx_error_with_text("failed to create claimant")?;

	create_claimable_balance_operation(to_be_redeemed_asset, to_be_redeemed_amount, vec![claimant])
}

pub fn create_claimable_balance_operation(
	asset: Asset,
	stroop_amount: StellarStroops,
	claimants: Vec<Claimant>,
) -> Result<Operation, Error> {
	let amount = StroopAmount(stroop_amount);

	Operation::new_create_claimable_balance(asset, amount, claimants).map_err_as_build_tx_error()
}

pub fn create_account_operation(
	destination_address: PublicKey,
	starting_stroop_amount: StellarStroops,
) -> Result<Operation, Error> {
	let amount = StroopAmount(starting_stroop_amount);

	Operation::new_create_account(destination_address, amount).map_err_as_build_tx_error()
}

pub fn create_payment_operation(
	destination_address: PublicKey,
	asset: Asset,
	stroop_amount: StellarStroops,
	source_address: PublicKey,
) -> Result<Operation, Error> {
	let stroop_amount = StroopAmount(stroop_amount);

	Operation::new_payment(destination_address, asset, stroop_amount)
		.map_err_as_build_tx_error()?
		.set_source_account(source_address)
		.map_err_as_build_tx_error_with_text("failed to set source account")
}

pub fn create_basic_spacewalk_stellar_transaction(
	request_id: [u8; 32],
	stroop_fee_per_operation: u32,
	public_key: PublicKey,
	next_sequence_number: SequenceNumber,
) -> Result<Transaction, Error> {
	let memo_text = Memo::MemoText(
		LimitedString::new(derive_shortened_request_id(&request_id))
			.map_err_as_build_tx_error_with_text("Invalid Hash")?,
	);

	Transaction::new(
		public_key,
		next_sequence_number,
		Some(stroop_fee_per_operation),
		Preconditions::PrecondNone,
		Some(memo_text),
	)
	.map_err_as_build_tx_error()
}
#[cfg(test)]
pub mod redeem_request_tests {
	use super::*;
	use crate::mock::{
		default_usdc_asset, public_key_from_encoding, secret_key_from_encoding,
	};
	use primitives::{stellar::SecretKey, CurrencyId};

	const INACTIVE_STELLAR_SECRET_KEY: &str =
		"SAOZUYCGHAHAHUN75JDPAEH7M42N64RN3AATZYB4X2MTXB6V7WV7O2IO";

	fn inactive_stellar_secretkey() -> SecretKey {
		secret_key_from_encoding(INACTIVE_STELLAR_SECRET_KEY)
	}

	const IS_PUBLIC_NETWORK: bool = false;

	const DEFAULT_SOURCE_PUBLIC_KEY: &str =
		"GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";
	const DEFAULT_DEST_PUBLIC_KEY: &str =
		"GBWYDOKJT5BEYJCCBJRHL54AMLRWDALR2NLEY7CKMPSOUKWVJTCFQFYI";

	fn default_testing_stellar_pubkeys() -> (PublicKey, PublicKey) {
		(
			public_key_from_encoding(DEFAULT_SOURCE_PUBLIC_KEY),
			public_key_from_encoding(DEFAULT_DEST_PUBLIC_KEY),
		)
	}

	pub fn create_account_merge_operation(
		destination_address: PublicKey,
		source_address: PublicKey,
	) -> Result<Operation, Error> {
		Operation::new_account_merge(destination_address)
			.map_err_as_build_tx_error()?
			.set_source_account(source_address)
			.map_err_as_build_tx_error()
	}

	#[tokio::test]
	async fn test_active_account_and_xlm_asset() {
		let client = reqwest::Client::new();
		let (source_pub_key, destination_pub_key) = default_testing_stellar_pubkeys();

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
				destination_pub_key,
				Asset::AssetTypeNative,
				10_000_000,
				source_pub_key
			)
			.expect("should not fail")
		);
	}

	#[tokio::test]
	async fn test_active_account_and_usdc_asset_with_trustline() {
		let client = reqwest::Client::new();
		let (source_pub_key, destination_pub_key) = default_testing_stellar_pubkeys();

		// existing account with non-XLM asset and has a trustline, use normal op
		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key.clone(),
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					default_usdc_asset(),
					10_000_000
				)
				.await
				.expect("should not fail"),
			create_payment_operation(
				destination_pub_key,
				default_usdc_asset(),
				10_000_000,
				source_pub_key
			)
			.expect("should not fail")
		);
	}

	#[tokio::test]
	async fn test_active_account_and_usdc_asset_no_trustline() {
		let client = reqwest::Client::new();
		let (source_pub_key, destination_pub_key) = default_testing_stellar_pubkeys();

		let unknown_asset = CurrencyId::try_from((
			"USDC",
			"GCXQXD3EDC7YILNAHRJLYR53MGQU4V5RXB7JHGPVS3HQHLBJDWCJHVIB",
		))
		.expect("should convert ok");
		let unknown_asset: Asset = unknown_asset.try_into().expect("should convert to Asset");

		// existing account with non-XLM asset BUT has NO trustline, use claimable balance
		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key.clone(),
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					unknown_asset.clone(),
					10_000_000
				)
				.await
				.expect("should not fail"),
			create_unconditional_claimable_balance_operation(
				destination_pub_key,
				unknown_asset,
				10_000_000,
			)
			.expect("should not fail")
		);
	}

	#[tokio::test]
	async fn test_inactive_account_and_xlm_asset_greater_than_equal_one() {
		let client = reqwest::Client::new();
		let source_pub_key = public_key_from_encoding(DEFAULT_SOURCE_PUBLIC_KEY);
		let destination_pub_key = inactive_stellar_secretkey().get_public().clone();

		// INactive account and XLM asset of value >=1, use create account op
		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key,
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					Asset::AssetTypeNative,
					10_000_000
				)
				.await
				.expect("should not fail"),
			create_account_operation(destination_pub_key, 10_000_000).expect("should not fail")
		);
	}

	#[tokio::test]
	async fn test_inactive_account_and_xlm_asset_less_than_one() {
		let client = reqwest::Client::new();
		let source_pub_key = public_key_from_encoding(DEFAULT_SOURCE_PUBLIC_KEY);
		let destination_pub_key = inactive_stellar_secretkey().get_public().clone();

		// INactive account but XLM asset of value < 1, use claimable balance
		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key,
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					Asset::AssetTypeNative,
					9_999_000
				)
				.await
				.expect("should not fail"),
			create_unconditional_claimable_balance_operation(
				destination_pub_key,
				Asset::AssetTypeNative,
				9_999_000,
			)
			.expect("should not fail")
		);
	}

	#[tokio::test]
	async fn test_inactive_account_and_usdc_asset() {
		let client = reqwest::Client::new();
		let source_pub_key = public_key_from_encoding(DEFAULT_SOURCE_PUBLIC_KEY);
		let destination_pub_key = inactive_stellar_secretkey().get_public().clone();

		let unknown_asset = CurrencyId::try_from((
			"USDC",
			"GCXQXD3EDC7YILNAHRJLYR53MGQU4V5RXB7JHGPVS3HQHLBJDWCJHVIB",
		))
		.expect("should convert ok");
		let unknown_asset: Asset = unknown_asset.try_into().expect("should convert to Asset");

		// INactive account and asset is NOT XLM, use claimable balance
		assert_eq!(
			client
				.create_payment_op_for_redeem_request(
					source_pub_key,
					destination_pub_key.clone(),
					IS_PUBLIC_NETWORK,
					unknown_asset.clone(),
					1_000_000
				)
				.await
				.expect("should not fail"),
			create_unconditional_claimable_balance_operation(
				destination_pub_key,
				unknown_asset,
				1_000_000,
			)
			.expect("should not fail")
		);
	}
}
