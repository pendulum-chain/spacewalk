use crate::helper::DEFAULT_TESTING_CURRENCY;
use async_trait::async_trait;
use frame_support::assert_ok;
use primitives::{CurrencyId, StellarStroops, H256};
use runtime::{
	integration::{
		assert_event, get_required_vault_collateral_for_issue, setup_provider, SubxtClient,
	},
	stellar::SecretKey,
	ExecuteRedeemEvent, IssuePallet, SpacewalkParachain, VaultId, VaultRegistryPallet,
};
use sp_keyring::AccountKeyring;
use sp_runtime::traits::StaticLookup;
use std::{sync::Arc, time::Duration};
use stellar_relay_lib::sdk::PublicKey;
use vault::{oracle::OracleAgent, ArcRwLock};
use wallet::StellarWallet;

pub fn default_destination() -> SecretKey {
	SecretKey::from_encoding(crate::helper::DEFAULT_TESTNET_DEST_SECRET_KEY).expect("Should work")
}

pub fn default_destination_as_binary() -> [u8; 32] {
	default_destination().get_public().clone().into_binary()
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
	let vault_id = VaultId::new(account.clone().into(), DEFAULT_TESTING_CURRENCY, wrapped_currency);

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

pub async fn register_vault_with_default_destination(
	items: Vec<(&SpacewalkParachain, &VaultId, u128)>,
) -> u128 {
	let public_key = default_destination().get_public().clone();

	register_vault(public_key, items).await
}

pub async fn register_vault_with_wallet(
	wallet: ArcRwLock<StellarWallet>,
	items: Vec<(&SpacewalkParachain, &VaultId, u128)>,
) -> u128 {
	let wallet_read = wallet.read().await;
	let public_key = wallet_read.get_public_key();

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

	let asset = primitives::AssetConversion::lookup(issue.asset).expect("Invalid asset");
	let stroop_amount = primitives::BalanceConversion::lookup(amount).expect("Invalid amount");

	let mut wallet_write = wallet.write().await;
	let destination_public_key = PublicKey::from_binary(issue.vault_stellar_public_key);

	let response = wallet_write
		.send_payment_to_address(
			destination_public_key,
			asset,
			stroop_amount,
			issue.issue_id.0,
			300,
		)
		.await
		.expect("Failed to send payment");

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
