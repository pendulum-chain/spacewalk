use crate::helper::{
	DEFAULT_TESTING_CURRENCY, DEFAULT_WRAPPED_CURRENCY_STELLAR_MAINNET,
	DEFAULT_WRAPPED_CURRENCY_STELLAR_TESTNET,
};
use async_trait::async_trait;
use frame_support::assert_ok;
use primitives::{stellar::Asset as StellarAsset, CurrencyId, StellarStroops, H256};
use runtime::{
	integration::{
		assert_event, get_required_vault_collateral_for_issue, setup_provider, SubxtClient,
	},
	ExecuteRedeemEvent, IssuePallet, SpacewalkParachain, VaultId, VaultRegistryPallet,
};
use sp_keyring::AccountKeyring;
use sp_runtime::traits::StaticLookup;
use std::{sync::Arc, time::Duration};
use stellar_relay_lib::sdk::{PublicKey, SecretKey};
use subxt::utils::AccountId32 as AccountId;
use vault::{oracle::OracleAgent, ArcRwLock};
use wallet::{error::Error, StellarWallet, TransactionResponse};

use wallet::keys::get_source_secret_key_from_env;

pub fn default_vault_stellar_secret(is_public_network: bool) -> SecretKey {
	SecretKey::from_encoding(get_source_secret_key_from_env(is_public_network))
		.expect("Should work")
}

pub fn default_vault_stellar_address_as_binary(is_public_network: bool) -> [u8; 32] {
	default_vault_stellar_secret(is_public_network)
		.get_public()
		.clone()
		.into_binary()
}

pub fn default_wrapped_currency(is_public_network: bool) -> CurrencyId {
	if is_public_network {
		DEFAULT_WRAPPED_CURRENCY_STELLAR_MAINNET
	} else {
		DEFAULT_WRAPPED_CURRENCY_STELLAR_TESTNET
	}
}

// A simple helper function to convert StellarStroops (i64) to the up-scaled u128
pub fn upscaled_compatible_amount(amount: StellarStroops) -> u128 {
	primitives::BalanceConversion::unlookup(amount)
}

#[async_trait]
pub trait SpacewalkParachainExt: VaultRegistryPallet {
	async fn register_vault_with_public_key(
		&self,
		vault_id: &VaultId,
		collateral: u128,
		public_key: crate::StellarPublicKey,
	) -> Result<(), runtime::Error> {
		self.register_public_key(public_key).await.unwrap();
		self.register_vault(vault_id, collateral).await.unwrap();
		Ok(())
	}
}

pub async fn create_vault(
	client: SubxtClient,
	account: AccountKeyring,
	wrapped_currency: CurrencyId,
) -> (VaultId, SpacewalkParachain) {
	let vault_id = VaultId::new(
		AccountId(account.to_account_id().clone().into()),
		DEFAULT_TESTING_CURRENCY,
		wrapped_currency,
	);

	let vault_provider = setup_provider(client, account).await;

	(vault_id, vault_provider)
}

pub async fn register_vault(
	destination_public_key: PublicKey,
	items: Vec<(&SpacewalkParachain, &VaultId, u128)>,
) -> u128 {
	let public_key = destination_public_key.into_binary();
	let mut vault_collateral = 0;
	for (provider, vault_id, issue_amount) in items {
		vault_collateral = get_required_vault_collateral_for_issue(
			provider,
			issue_amount,
			vault_id.wrapped_currency(),
			vault_id.collateral_currency(),
		)
		.await;

		assert_ok!(
			provider
				.register_vault_with_public_key(vault_id, vault_collateral, public_key)
				.await
		);
	}

	vault_collateral
}

pub async fn register_vault_with_default_stellar_account(
	items: Vec<(&SpacewalkParachain, &VaultId, u128)>,
	is_public_network: bool,
) -> u128 {
	let public_key = default_vault_stellar_secret(is_public_network).get_public().clone();

	register_vault(public_key, items).await
}

pub async fn register_vault_with_wallet(
	wallet: ArcRwLock<StellarWallet>,
	items: Vec<(&SpacewalkParachain, &VaultId, u128)>,
) -> u128 {
	let wallet_read = wallet.read().await;
	let public_key = wallet_read.public_key();

	let vault_collateral = register_vault(public_key, items).await;

	drop(wallet_read);

	vault_collateral
}

pub async fn assert_execute_redeem_event(
	duration: Duration,
	parachain_rpc: SpacewalkParachain,
	redeem_id: H256,
) -> ExecuteRedeemEvent {
	assert_event::<ExecuteRedeemEvent, _>(duration, parachain_rpc, |x| x.redeem_id == redeem_id)
		.await
}

pub async fn send_payment_to_address(
	wallet: ArcRwLock<StellarWallet>,
	destination_address: PublicKey,
	asset: StellarAsset,
	stroop_amount: StellarStroops,
	request_id: [u8; 32],
	is_payment_for_redeem_request: bool,
) -> Result<TransactionResponse, Error> {
	let response;
	loop {
		let result = wallet
			.write()
			.await
			.send_payment_to_address(
				destination_address.clone(),
				asset.clone(),
				stroop_amount,
				request_id,
				is_payment_for_redeem_request,
			)
			.await;

		match &result {
			// if the error is `tx_bad_seq` perform the process again
			Err(Error::HorizonSubmissionError { reason, .. }) if reason.contains("tx_bad_seq") =>
				continue,
			_ => {
				response = result;
				break;
			},
		}
	}

	drop(wallet);

	response
}

/// request, pay and execute an issue
pub async fn assert_issue(
	parachain_rpc: &SpacewalkParachain,
	wallet: ArcRwLock<StellarWallet>,
	vault_id: &VaultId,
	amount: u128,
	oracle_agent: Arc<OracleAgent>,
) {
	let issue = parachain_rpc
		.request_issue(amount, vault_id)
		.await
		.expect("Failed to request issue");

	let asset = primitives::AssetConversion::lookup(*issue.asset).expect("Invalid asset");
	let stroop_amount = primitives::BalanceConversion::lookup(amount).expect("Invalid amount");

	let destination_public_key = PublicKey::from_binary(issue.vault_stellar_public_key);

	let response = send_payment_to_address(
		wallet,
		destination_public_key,
		asset,
		stroop_amount,
		issue.issue_id.0,
		false,
	)
	.await
	.expect("should return ok");

	let slot = response.ledger as u64;

	// Loop pending proofs until it is ready
	let proof = oracle_agent.get_proof(slot).await.expect("Proof should be available");
	let tx_envelope_xdr_encoded = response.envelope_xdr;
	let (envelopes_xdr_encoded, tx_set_xdr_encoded) = proof.encode();

	parachain_rpc
		.execute_issue(
			issue.issue_id,
			tx_envelope_xdr_encoded.as_slice(),
			envelopes_xdr_encoded.as_bytes(),
			tx_set_xdr_encoded.as_bytes(),
		)
		.await
		.expect("Failed to execute issue");
}
