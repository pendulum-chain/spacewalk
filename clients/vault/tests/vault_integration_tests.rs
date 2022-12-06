use std::{collections::HashMap, convert::TryInto, sync::Arc, time::Duration};

use async_trait::async_trait;
use frame_support::assert_ok;
use futures::{
	future::{join, join3, join4},
	Future, FutureExt,
};
use sp_keyring::AccountKeyring;
use sp_runtime::traits::StaticLookup;
use tokio::{
	sync::{mpsc, RwLock},
	time::sleep,
};

use primitives::{issue::IssueRequest, H256};
use runtime::{
	integration::*, types::*, CurrencyId::Token, FixedPointNumber, FixedU128, IssuePallet,
	SpacewalkParachain, UtilFuncs, VaultRegistryPallet,
};
use stellar_relay_lib::sdk::{Hash, PublicKey, SecretKey, XdrCodec};
use vault::{
	oracle::{create_handler, Proof, ProofExt, ProofStatus},
	service::IssueFilter,
	Event as CancellationEvent, VaultIdManager,
};
use wallet::{types::Watcher, StellarWallet};

const TIMEOUT: Duration = Duration::from_secs(60);

// Be careful when changing these values because they are used in the parachain genesis config
// and only for some combination of them, secure collateralization thresholds are set.
const DEFAULT_NATIVE_CURRENCY: CurrencyId = Token(TokenSymbol::PEN);
const DEFAULT_TESTING_CURRENCY: CurrencyId = Token(TokenSymbol::DOT);
const DEFAULT_WRAPPED_CURRENCY: CurrencyId = CurrencyId::AlphaNum4 {
	code: *b"USDC",
	issuer: [
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
};

const STELLAR_VAULT_SECRET_KEY: &str = "SB6WHKIU2HGVBRNKNOEOQUY4GFC4ZLG5XPGWLEAHTIZXBXXYACC76VSQ";

type StellarPublicKey = [u8; 32];

#[async_trait]
trait SpacewalkParachainExt {
	async fn register_vault_with_public_key(
		&self,
		vault_id: &VaultId,
		collateral: u128,
		public_key: StellarPublicKey,
	) -> Result<(), runtime::Error>;
}

#[async_trait]
impl SpacewalkParachainExt for SpacewalkParachain {
	async fn register_vault_with_public_key(
		&self,
		vault_id: &VaultId,
		collateral: u128,
		public_key: StellarPublicKey,
	) -> Result<(), runtime::Error> {
		self.register_public_key(public_key).await.unwrap();
		self.register_vault(vault_id, collateral).await.unwrap();
		Ok(())
	}
}

async fn test_with<F, R>(execute: impl FnOnce(SubxtClient) -> F) -> R
where
	F: Future<Output = R>,
{
	service::init_subscriber();
	let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;

	// Has to be Bob because he is set as `authorized_oracle` in the genesis config
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;

	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_TESTING_CURRENCY,
		FixedU128::from(100000000),
	)
	.await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_NATIVE_CURRENCY,
		FixedU128::saturating_from_rational(1u128, 100u128),
	)
	.await;
	set_stellar_fees(&parachain_rpc, FixedU128::from(1)).await;

	execute(client).await
}

async fn test_with_vault<F, R>(
	execute: impl FnOnce(SubxtClient, VaultId, SpacewalkParachain) -> F,
) -> R
where
	F: Future<Output = R>,
{
	service::init_subscriber();
	let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;

	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_TESTING_CURRENCY,
		FixedU128::from(100000000),
	)
	.await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_NATIVE_CURRENCY,
		FixedU128::saturating_from_rational(1u128, 100u128),
	)
	.await;
	set_stellar_fees(&parachain_rpc, FixedU128::from(1)).await;

	let vault_provider = setup_provider(client.clone(), AccountKeyring::Charlie).await;

	let account_id = AccountKeyring::Charlie.to_raw_public();
	// Convert to the subxt AccountId type because unfortunately there is a version mismatch between
	// the sp_xxx dependencies subxt uses and the ones we use
	let account_id = subxt::ext::sp_runtime::AccountId32::from(account_id);

	let vault_id = VaultId::new(account_id, DEFAULT_TESTING_CURRENCY, DEFAULT_WRAPPED_CURRENCY);

	execute(client, vault_id, vault_provider).await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_issue_overpayment_succeeds() {
	test_with_vault(|client, vault_id, vault_provider| async move {
		let relayer_provider = setup_provider(client.clone(), AccountKeyring::Bob).await;
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let is_public_network = false;
		let wallet = StellarWallet::from_secret_encoded(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			is_public_network,
		)
		.unwrap();
		let public_key = wallet.get_public_key_raw();

		// Already start listening for scp messages
		let scp_handler = vault::inner_create_handler(wallet.get_secret_key(), is_public_network)
			.await
			.expect("Failed to create scp handler");

		let issue_amount = 100000;
		let over_payment_factor = 3;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount * over_payment_factor,
			vault_id.collateral_currency(),
		)
		.await;

		assert_ok!(
			vault_provider
				.register_vault_with_public_key(&vault_id, vault_collateral, public_key)
				.await
		);

		// This call returns a RequestIssueEvent, not the IssueRequest from primitives
		let issue = user_provider
			.request_issue(issue_amount, &vault_id)
			.await
			.expect("Requesting issue failed");

		// Send a payment to the destination of the issue request (ie the targeted vault's
		// stellar account)
		let stroop_amount = (issue.amount + issue.fee) * over_payment_factor;
		let destination_public_key = PublicKey::from_binary(issue.vault_stellar_public_key);
		let stellar_asset =
			primitives::AssetConversion::lookup(issue.asset).expect("Asset not found");
		let memo_hash = issue.issue_id.0;

		let (transaction_response, tx_envelope) = wallet
			.send_payment_to_address(
				destination_public_key,
				stellar_asset,
				stroop_amount.try_into().unwrap(),
				memo_hash,
			)
			.await
			.expect("Sending payment failed");

		assert!(transaction_response.successful);

		// Loop pending proofs until it is ready
		let mut proof: Option<Proof> = None;
		with_timeout(
			async {
				loop {
					let proof_status = scp_handler
						.proof_operations()
						.get_proof(transaction_response.ledger as u64)
						.await
						.expect("Failed to get proof");

					match proof_status {
						ProofStatus::Proof(p) => {
							proof = Some(p);
							break
						},
						ProofStatus::LackingEnvelopes => {},
						ProofStatus::NoEnvelopesFound => {},
						ProofStatus::NoTxSetFound => {},
						ProofStatus::WaitForTxSet => {},
					}

					// Wait a bit before trying again
					sleep(Duration::from_secs(3)).await;
				}
			},
			Duration::from_secs(60),
		)
		.await;

		let proof = proof.expect("Proof should be available");
		let tx_envelope_xdr_encoded = tx_envelope.to_xdr();
		let tx_envelope_xdr_encoded = base64::encode(tx_envelope_xdr_encoded);
		let (envelopes_xdr_encoded, tx_set_xdr_encoded) = proof.encode();

		join(
			assert_event::<EndowedEvent, _>(TIMEOUT, user_provider.clone(), |x| {
				if &x.who == user_provider.get_account_id() {
					assert_eq!(x.amount, issue.amount * over_payment_factor);
					true
				} else {
					false
				}
			}),
			user_provider
				.execute_issue(
					issue.issue_id,
					tx_envelope_xdr_encoded.as_bytes(),
					envelopes_xdr_encoded.as_bytes(),
					tx_set_xdr_encoded.as_bytes(),
				)
				.map(Result::unwrap),
		)
		.await;
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_automatic_issue_execution_succeeds() {
	test_with_vault(|client, vault1_id, _vault1_provider| async move {
		let relayer_provider = setup_provider(client.clone(), AccountKeyring::Bob).await;
		let vault1_provider = setup_provider(client.clone(), AccountKeyring::Charlie).await;
		let vault2_provider = setup_provider(client.clone(), AccountKeyring::Eve).await;

		// This conversion is necessary for now because subxt uses newer versions of the sp_xxx
		// dependencies
		let eve_account =
			subxt::ext::sp_runtime::AccountId32::from(AccountKeyring::Eve.to_raw_public());
		let vault2_id =
			VaultId::new(eve_account, DEFAULT_TESTING_CURRENCY, DEFAULT_WRAPPED_CURRENCY);
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let is_public_network = false;
		let wallet = StellarWallet::from_secret_encoded(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			is_public_network,
		)
		.unwrap();
		let wallet = Arc::new(wallet);

		let vault_ids = vec![vault2_id.clone()];
		let vault_id_manager =
			VaultIdManager::from_map(vault2_provider.clone(), wallet.clone(), vault_ids);

		let issue_amount = 100000;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault1_provider,
			issue_amount,
			vault1_id.collateral_currency(),
		)
		.await;

		assert_ok!(
			vault1_provider
				.register_vault_with_public_key(
					&vault1_id,
					vault_collateral,
					wallet.get_public_key_raw(),
				)
				.await
		);
		assert_ok!(
			vault2_provider
				.register_vault_with_public_key(
					&vault2_id,
					vault_collateral,
					wallet.get_public_key_raw(),
				)
				.await
		);

		let fut_user = async {
			// The account of the 'user_provider' is used to request a new issue that
			// has to be executed by vault1
			let issue = user_provider.request_issue(issue_amount, &vault1_id).await.unwrap();
			tracing::warn!("REQUESTED ISSUE: {:?}", issue);

			let destination_public_key = PublicKey::from_binary(issue.vault_stellar_public_key);
			let stroop_amount = (issue.amount + issue.fee) as i64;
			let stellar_asset =
				primitives::AssetConversion::lookup(issue.asset).expect("Asset not found");
			let memo_hash = issue.issue_id.0;

			let result = wallet
				.send_payment_to_address(
					destination_public_key,
					stellar_asset,
					stroop_amount,
					memo_hash,
				)
				.await;
			assert!(result.is_ok());

			tracing::warn!("Sent payment to address. Ledger is {:?}", result.unwrap().0.ledger);

			// wait for vault1 to execute this issue
			assert_event::<ExecuteIssueEvent, _>(TIMEOUT, user_provider.clone(), move |x| {
				x.vault_id == vault1_id.clone()
			})
			.await;
		};

		let slot_tx_env_map: Arc<RwLock<HashMap<u32, String>>> =
			Arc::new(RwLock::new(HashMap::new()));

		let issue_filter = IssueFilter::new(&wallet.get_public_key()).expect("Invalid filter");
		let handler = vault::inner_create_handler(wallet.get_secret_key(), is_public_network)
			.await
			.expect("Failed to create handler");
		let watcher = Arc::new(RwLock::new(handler.create_watcher()));
		let proof_ops = Arc::new(RwLock::new(handler.proof_operations()));

		let issue_set = Arc::new(RwLock::new(IssueRequestsMap::new()));
		let (issue_event_tx, _issue_event_rx) = mpsc::channel::<CancellationEvent>(16);
		let service = join4(
			vault::service::listen_for_new_transactions(
				wallet.get_public_key(),
				wallet.is_public_network(),
				watcher.clone(),
				slot_tx_env_map.clone(),
				issue_set.clone(),
				issue_filter,
			),
			vault::service::listen_for_issue_requests(
				vault2_provider.clone(),
				wallet.get_public_key(),
				issue_set.clone(),
			),
			vault::service::process_issues_with_proofs(
				vault2_provider.clone(),
				proof_ops.clone(),
				slot_tx_env_map.clone(),
				issue_set.clone(),
			),
			periodically_produce_blocks(vault2_provider.clone()),
		);

		test_service(service, fut_user).await;
	})
	.await;
}
