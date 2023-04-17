use std::{collections::HashMap, convert::TryInto, sync::Arc, time::Duration};

use async_trait::async_trait;
use frame_support::assert_ok;
use futures::{
	channel::mpsc,
	future::{join, join3, join4},
	Future, FutureExt, SinkExt,
};
use lazy_static::lazy_static;
use serial_test::serial;
use sp_keyring::AccountKeyring;
use sp_runtime::traits::StaticLookup;
use tokio::{sync::RwLock, time::sleep};

use primitives::{StellarStroops, H256};
use runtime::{
	integration::*, types::*, FixedPointNumber, FixedU128, IssuePallet, RedeemPallet,
	ReplacePallet, ShutdownSender, SpacewalkParachain, SudoPallet, UtilFuncs, VaultRegistryPallet,
};
use stellar_relay_lib::{sdk::PublicKey, StellarOverlayConfig};

use vault::{
	oracle::{get_test_secret_key, get_test_stellar_relay_config, start_oracle_agent, OracleAgent},
	service::IssueFilter,
	ArcRwLock, Event as CancellationEvent, VaultIdManager,
};
use wallet::StellarWallet;

const TIMEOUT: Duration = Duration::from_secs(60);

// Be careful when changing these values because they are used in the parachain genesis config
// and only for some combination of them, secure collateralization thresholds are set.
const DEFAULT_TESTING_CURRENCY: CurrencyId = CurrencyId::XCM(0);
const DEFAULT_WRAPPED_CURRENCY: CurrencyId = CurrencyId::AlphaNum4(
	*b"USDC",
	[
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
);

lazy_static! {
	static ref CFG: StellarOverlayConfig = get_test_stellar_relay_config(false);
	static ref SECRET_KEY: String = get_test_secret_key(false);
}

// A simple helper function to convert StellarStroops (i64) to the up-scaled u128
fn upscaled_compatible_amount(amount: StellarStroops) -> u128 {
	primitives::BalanceConversion::unlookup(amount)
}

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

async fn assert_execute_redeem_event(
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
	wallet: Arc<RwLock<StellarWallet>>,
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
	let destination_address = wallet_write.get_public_key();
	let response = wallet_write
		.send_payment_to_address(destination_address, asset, stroop_amount, issue.issue_id.0, 300)
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

async fn test_with<F, R>(execute: impl FnOnce(SubxtClient, ArcRwLock<StellarWallet>) -> F) -> R
where
	F: Future<Output = R>,
{
	service::init_subscriber();
	let (client, tmp_dir) = default_provider_client(AccountKeyring::Alice).await;

	// Has to be Bob because he is set as `authorized_oracle` in the genesis config
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;

	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_TESTING_CURRENCY,
		// Set exchange rate to 1:1 with USD
		FixedU128::saturating_from_rational(1u128, 1u128),
	)
	.await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_WRAPPED_CURRENCY,
		// Set exchange rate to 10:1 with USD
		FixedU128::saturating_from_rational(1u128, 10u128),
	)
	.await;

	let path = tmp_dir.path().to_str().expect("should return a string").to_string();
	let wallet = Arc::new(RwLock::new(
		StellarWallet::from_secret_encoded_with_cache(&SECRET_KEY, CFG.is_public_network(), path)
			.unwrap(),
	));

	execute(client, wallet).await
}

async fn test_with_vault<F, R>(
	execute: impl FnOnce(
		SubxtClient,
		ArcRwLock<StellarWallet>,
		Arc<OracleAgent>,
		VaultId,
		SpacewalkParachain,
	) -> F,
) -> R
where
	F: Future<Output = R>,
{
	service::init_subscriber();
	let (client, tmp_dir) = default_provider_client(AccountKeyring::Alice).await;

	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_TESTING_CURRENCY,
		// Set exchange rate to 1:1 with USD
		FixedU128::saturating_from_rational(1u128, 1u128),
	)
	.await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_WRAPPED_CURRENCY,
		// Set exchange rate to 10:1 with USD
		FixedU128::saturating_from_rational(1u128, 10u128),
	)
	.await;

	let vault_provider = setup_provider(client.clone(), AccountKeyring::Charlie).await;
	let vault_id = VaultId::new(
		AccountKeyring::Charlie.into(),
		DEFAULT_TESTING_CURRENCY,
		DEFAULT_WRAPPED_CURRENCY,
	);

	let path = tmp_dir.path().to_str().expect("should return a string").to_string();
	let wallet = Arc::new(RwLock::new(
		StellarWallet::from_secret_encoded_with_cache(&SECRET_KEY, CFG.is_public_network(), path)
			.unwrap(),
	));

	let oracle_agent = start_oracle_agent(CFG.clone(), &SECRET_KEY)
		.await
		.expect("failed to start agent");
	let oracle_agent = Arc::new(oracle_agent);

	execute(client, wallet, oracle_agent, vault_id, vault_provider).await
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_redeem_succeeds() {
	test_with_vault(|client, wallet, oracle_agent, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let vault_ids = vec![vault_id.clone()];
		let vault_id_manager =
			VaultIdManager::from_map(vault_provider.clone(), wallet.clone(), vault_ids);

		// We issue 1 (spacewalk-chain) unit
		let issue_amount = CurrencyId::Native.one();
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount,
			vault_id.wrapped_currency(),
			vault_id.collateral_currency(),
		)
		.await;

		let wallet_read = wallet.read().await;
		assert_ok!(
			vault_provider
				.register_vault_with_public_key(
					&vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw()
				)
				.await
		);
		drop(wallet_read);

		let shutdown_tx = ShutdownSender::new();

		assert_issue(&user_provider, wallet.clone(), &vault_id, issue_amount, oracle_agent.clone())
			.await;

		test_service(
			vault::service::listen_for_redeem_requests(
				shutdown_tx,
				vault_provider.clone(),
				vault_id_manager,
				Duration::from_secs(0),
				oracle_agent,
			),
			async {
				let wallet_read = wallet.read().await;
				let address = wallet_read.get_public_key_raw();
				drop(wallet_read);
				// We redeem half of what we issued
				let redeem_id = user_provider
					.request_redeem(issue_amount / 2, address, &vault_id)
					.await
					.unwrap();
				assert_execute_redeem_event(TIMEOUT, user_provider, redeem_id).await;
			},
		)
		.await;
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_replace_succeeds() {
	test_with_vault(|client, wallet, oracle_agent, old_vault_id, old_vault_provider| async move {
		let new_vault_provider = setup_provider(client.clone(), AccountKeyring::Eve).await;
		let new_vault_id = VaultId::new(
			AccountKeyring::Eve.into(),
			DEFAULT_TESTING_CURRENCY,
			DEFAULT_WRAPPED_CURRENCY,
		);
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let vault_ids = vec![new_vault_id.clone()].into_iter().collect();
		let _vault_id_manager =
			VaultIdManager::from_map(new_vault_provider.clone(), wallet.clone(), vault_ids);
		let vault_ids = vec![old_vault_id.clone(), new_vault_id.clone()].into_iter().collect();
		let vault_id_manager =
			VaultIdManager::from_map(old_vault_provider.clone(), wallet.clone(), vault_ids);

		let issue_amount = upscaled_compatible_amount(100);
		let vault_collateral = get_required_vault_collateral_for_issue(
			&old_vault_provider,
			issue_amount,
			old_vault_id.wrapped_currency(),
			old_vault_id.collateral_currency(),
		)
		.await;

		let wallet_read = wallet.read().await;
		assert_ok!(
			old_vault_provider
				.register_vault_with_public_key(
					&old_vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw()
				)
				.await
		);
		assert_ok!(
			new_vault_provider
				.register_vault_with_public_key(
					&new_vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw()
				)
				.await
		);
		drop(wallet_read);

		assert_issue(
			&user_provider,
			wallet.clone(),
			&old_vault_id,
			issue_amount,
			oracle_agent.clone(),
		)
		.await;

		let shutdown_tx = ShutdownSender::new();
		let (replace_event_tx, _) = mpsc::channel::<CancellationEvent>(16);
		test_service(
			join(
				vault::service::listen_for_replace_requests(
					new_vault_provider.clone(),
					vault_id_manager.clone(),
					replace_event_tx.clone(),
					true,
				),
				vault::service::listen_for_accept_replace(
					shutdown_tx.clone(),
					old_vault_provider.clone(),
					vault_id_manager.clone(),
					Duration::from_secs(0),
					oracle_agent.clone(),
				),
			),
			async {
				old_vault_provider.request_replace(&old_vault_id, issue_amount).await.unwrap();

				assert_event::<AcceptReplaceEvent, _>(TIMEOUT, old_vault_provider.clone(), |e| {
					assert_eq!(e.old_vault_id, old_vault_id);
					assert_eq!(e.new_vault_id, new_vault_id);
					true
				})
				.await;
				assert_event::<ExecuteReplaceEvent, _>(TIMEOUT, old_vault_provider.clone(), |e| {
					assert_eq!(e.old_vault_id, old_vault_id);
					assert_eq!(e.new_vault_id, new_vault_id);
					true
				})
				.await;
			},
		)
		.await;
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_withdraw_replace_succeeds() {
	test_with_vault(|client, wallet, oracle_agent, old_vault_id, old_vault_provider| async move {
		let new_vault_provider = setup_provider(client.clone(), AccountKeyring::Eve).await;
		let new_vault_id = VaultId::new(
			AccountKeyring::Eve.into(),
			DEFAULT_TESTING_CURRENCY,
			DEFAULT_WRAPPED_CURRENCY,
		);
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let issue_amount = upscaled_compatible_amount(100);
		let vault_collateral = get_required_vault_collateral_for_issue(
			&old_vault_provider,
			issue_amount,
			old_vault_id.wrapped_currency(),
			old_vault_id.collateral_currency(),
		)
		.await;
		let wallet_read = wallet.read().await;
		assert_ok!(
			old_vault_provider
				.register_vault_with_public_key(
					&old_vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw()
				)
				.await
		);
		assert_ok!(
			new_vault_provider
				.register_vault_with_public_key(
					&new_vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw()
				)
				.await
		);
		drop(wallet_read);

		assert_issue(
			&user_provider,
			wallet.clone(),
			&old_vault_id,
			issue_amount,
			oracle_agent.clone(),
		)
		.await;

		join(
			old_vault_provider
				.request_replace(&old_vault_id, issue_amount)
				.map(Result::unwrap),
			assert_event::<RequestReplaceEvent, _>(TIMEOUT, old_vault_provider.clone(), |_| true),
		)
		.await;

		join(
			old_vault_provider
				.withdraw_replace(&old_vault_id, issue_amount)
				.map(Result::unwrap),
			assert_event::<WithdrawReplaceEvent, _>(TIMEOUT, old_vault_provider.clone(), |e| {
				assert_eq!(e.old_vault_id, old_vault_id);
				true
			}),
		)
		.await;

		let address = [2u8; 32];
		assert!(new_vault_provider
			.accept_replace(&new_vault_id, &old_vault_id, 1u32.into(), vault_collateral, address)
			.await
			.is_err());
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_cancel_scheduler_succeeds() {
	// tests cancellation of issue, redeem and replace.
	// issue and replace cancellation is tested through the vault's cancellation service.
	// cancel_redeem is called manually
	test_with_vault(|client, wallet, oracle_agent, old_vault_id, old_vault_provider| async move {
		let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;

		let root_provider = setup_provider(client.clone(), AccountKeyring::Alice).await;
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;
		let new_vault_provider = setup_provider(client.clone(), AccountKeyring::Eve).await;
		let new_vault_id = VaultId::new(
			AccountKeyring::Eve.into(),
			DEFAULT_TESTING_CURRENCY,
			DEFAULT_WRAPPED_CURRENCY,
		);

		let issue_amount = upscaled_compatible_amount(200);
		let vault_collateral = get_required_vault_collateral_for_issue(
			&old_vault_provider,
			issue_amount * 10,
			old_vault_id.wrapped_currency(),
			old_vault_id.collateral_currency(),
		)
		.await;

		let wallet_read = wallet.read().await;
		assert_ok!(
			old_vault_provider
				.register_vault_with_public_key(
					&old_vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw()
				)
				.await
		);
		assert_ok!(
			new_vault_provider
				.register_vault_with_public_key(
					&new_vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw()
				)
				.await
		);
		drop(wallet_read);

		assert_issue(
			&user_provider,
			wallet.clone(),
			&old_vault_id,
			issue_amount,
			oracle_agent.clone(),
		)
		.await;

		// set low timeout periods
		assert_ok!(root_provider.set_issue_period(1).await);
		assert_ok!(root_provider.set_replace_period(1).await);
		assert_ok!(root_provider.set_redeem_period(1).await);

		let (issue_cancellation_event_tx, issue_cancellation_rx) =
			mpsc::channel::<CancellationEvent>(16);
		let (replace_cancellation_event_tx, replace_cancellation_rx) =
			mpsc::channel::<CancellationEvent>(16);

		let block_listener = new_vault_provider.clone();
		let issue_set = Arc::new(RwLock::new(IssueRequestsMap::new()));
		let memos_to_issue_ids = Arc::new(RwLock::new(IssueIdLookup::new()));

		let wallet_read = wallet.read().await;
		let issue_request_listener = vault::service::listen_for_issue_requests(
			new_vault_provider.clone(),
			wallet_read.get_public_key(),
			issue_cancellation_event_tx.clone(),
			issue_set.clone(),
			memos_to_issue_ids.clone(),
		);
		drop(wallet_read);

		let issue_cancellation_scheduler = vault::service::CancellationScheduler::new(
			new_vault_provider.clone(),
			new_vault_provider.get_current_chain_height().await.unwrap(),
			new_vault_provider.get_account_id().clone(),
		);
		let replace_cancellation_scheduler = vault::service::CancellationScheduler::new(
			new_vault_provider.clone(),
			new_vault_provider.get_current_chain_height().await.unwrap(),
			new_vault_provider.get_account_id().clone(),
		);
		let issue_canceller = issue_cancellation_scheduler
			.handle_cancellation::<vault::service::IssueCanceller>(issue_cancellation_rx);
		let replace_canceller = replace_cancellation_scheduler
			.handle_cancellation::<vault::service::ReplaceCanceller>(replace_cancellation_rx);

		let parachain_block_listener = async {
			let issue_block_tx = &issue_cancellation_event_tx.clone();
			let replace_block_tx = &replace_cancellation_event_tx.clone();

			block_listener
				.clone()
				.on_event::<UpdateActiveBlockEvent, _, _, _>(
					|event| async move {
						assert_ok!(
							issue_block_tx
								.clone()
								.send(CancellationEvent::ParachainBlock(event.block_number))
								.await
						);
						assert_ok!(
							replace_block_tx
								.clone()
								.send(CancellationEvent::ParachainBlock(event.block_number))
								.await
						);
					},
					|_err| (),
				)
				.await
				.unwrap();
		};

		test_service(
			join4(
				issue_canceller.map(Result::unwrap),
				replace_canceller.map(Result::unwrap),
				issue_request_listener.map(Result::unwrap),
				parachain_block_listener,
			),
			async {
				let address = [2u8; 32];

				// setup the to-be-cancelled redeem
				let redeem_id = user_provider
					.request_redeem(upscaled_compatible_amount(100), address, &old_vault_id)
					.await
					.unwrap();

				join3(
					async {
						// setup the to-be-cancelled replace
						assert_ok!(
							old_vault_provider
								.request_replace(&old_vault_id, issue_amount / 2)
								.await
						);
						assert_ok!(
							new_vault_provider
								.accept_replace(
									&new_vault_id,
									&old_vault_id,
									10000000u32.into(),
									0u32.into(),
									address
								)
								.await
						);
						assert_ok!(
							replace_cancellation_event_tx
								.clone()
								.send(CancellationEvent::Opened)
								.await
						);

						// setup the to-be-cancelled issue
						assert_ok!(user_provider.request_issue(issue_amount, &new_vault_id).await);

						// Create two new blocks so that the current requests expire (since we set
						// the periods to 1 before)
						parachain_rpc.manual_seal().await;
						sleep(Duration::from_secs(10)).await;
						parachain_rpc.manual_seal().await;
					},
					assert_event::<CancelIssueEvent, _>(
						Duration::from_secs(120),
						user_provider.clone(),
						|_| true,
					),
					assert_event::<CancelReplaceEvent, _>(
						Duration::from_secs(120),
						user_provider.clone(),
						|_| true,
					),
				)
				.await;

				// now make sure we can cancel the redeem
				assert_ok!(user_provider.cancel_redeem(redeem_id, true).await);
			},
		)
		.await;
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_issue_cancel_succeeds() {
	test_with_vault(|client, wallet, _, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;
		let issue_set = Arc::new(RwLock::new(IssueRequestsMap::new()));
		let memos_to_issue_ids = Arc::new(RwLock::new(IssueIdLookup::new()));

		let issue_filter =
			IssueFilter::new(&wallet.read().await.get_public_key()).expect("Invalid filter");

		let issue_amount = upscaled_compatible_amount(100);
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount,
			vault_id.wrapped_currency(),
			vault_id.collateral_currency(),
		)
		.await;

		assert_ok!(
			vault_provider
				.register_vault_with_public_key(
					&vault_id,
					vault_collateral,
					wallet.read().await.get_public_key_raw(),
				)
				.await
		);

		let fut_user = async {
			// The account of the 'user_provider' is used to request a new issue that
			// will be canceled in the next step
			let issue = user_provider.request_issue(issue_amount, &vault_id).await.unwrap();
			// First await the event that the issue has been requested
			assert_event::<RequestIssueEvent, _>(TIMEOUT, user_provider.clone(), |_| true).await;
			assert_eq!(issue_set.read().await.len(), 1);
			assert_eq!(memos_to_issue_ids.read().await.len(), 1);

			match user_provider.cancel_issue(issue.issue_id).await {
				Ok(_) => {
					assert!(true);
				},
				Err(e) => {
					panic!("Failed to Cancel Issue: {:?}", e);
				},
			}

			// wait for the issue to be canceled
			assert_event::<CancelIssueEvent, _>(TIMEOUT, user_provider.clone(), |_| true).await;
			assert!(issue_set.read().await.is_empty());
			assert!(memos_to_issue_ids.read().await.is_empty());
		};

		let slot_tx_env_map = Arc::new(RwLock::new(HashMap::new()));

		let (issue_event_tx, _issue_event_rx) = mpsc::channel::<CancellationEvent>(16);
		let wallet_read = wallet.read().await;
		let service = join3(
			vault::service::listen_for_new_transactions(
				wallet_read.get_public_key(),
				wallet_read.is_public_network(),
				slot_tx_env_map.clone(),
				issue_set.clone(),
				memos_to_issue_ids.clone(),
				issue_filter,
			),
			vault::service::listen_for_issue_requests(
				vault_provider.clone(),
				wallet_read.get_public_key(),
				issue_event_tx,
				issue_set.clone(),
				memos_to_issue_ids.clone(),
			),
			vault::service::listen_for_issue_cancels(
				vault_provider.clone(),
				issue_set.clone(),
				memos_to_issue_ids.clone(),
			),
		);

		test_service(service, fut_user).await;
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_issue_overpayment_succeeds() {
	test_with_vault(|client, wallet, oracle_agent, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let public_key = wallet.read().await.get_public_key_raw();

		let issue_amount = upscaled_compatible_amount(100);
		let over_payment_factor = 3;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount * over_payment_factor,
			vault_id.wrapped_currency(),
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
		let stroop_amount =
			primitives::BalanceConversion::lookup((issue.amount + issue.fee) * over_payment_factor)
				.expect("Conversion should not fail");
		let destination_public_key = PublicKey::from_binary(issue.vault_stellar_public_key);
		let stellar_asset =
			primitives::AssetConversion::lookup(issue.asset).expect("Asset not found");

		let transaction_response = wallet
			.write()
			.await
			.send_payment_to_address(
				destination_public_key,
				stellar_asset,
				stroop_amount.try_into().unwrap(),
				issue.issue_id.0,
				300,
			)
			.await
			.expect("Sending payment failed");

		assert!(transaction_response.successful);

		let slot = transaction_response.ledger as u64;

		// Loop pending proofs until it is ready
		let proof = oracle_agent.get_proof(slot).await.expect("Proof should be available");
		let tx_envelope_xdr_encoded = transaction_response.envelope_xdr;
		let (envelopes_xdr_encoded, tx_set_xdr_encoded) = proof.encode();

		join(
			assert_event::<EndowedEvent, _>(TIMEOUT, user_provider.clone(), |x| {
				if &x.who == user_provider.get_account_id() {
					// Overpaying will make the issue pallet recalculate the amount and fee for the
					// higher amount. With the up-scaled and overpaid amount of 300_00000, the
					// resulting fee will be 300_00000 * 0.001 = 30000
					let fee = 30_000;
					assert_eq!(x.amount, issue.amount * over_payment_factor - fee);
					true
				} else {
					false
				}
			}),
			user_provider
				.execute_issue(
					issue.issue_id,
					&tx_envelope_xdr_encoded,
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
#[serial]
async fn test_automatic_issue_execution_succeeds() {
	test_with_vault(|client, wallet, oracle_agent, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let issue_amount = upscaled_compatible_amount(1000);
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount,
			vault_id.wrapped_currency(),
			vault_id.collateral_currency(),
		)
		.await;

		let wallet_read = wallet.read().await;
		assert_ok!(
			vault_provider
				.register_vault_with_public_key(
					&vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw(),
				)
				.await
		);
		drop(wallet_read);

		let fut_user = async {
			// The account of the 'user_provider' is used to request a new issue that
			// has to be executed by the vault_provider
			let issue = user_provider.request_issue(issue_amount, &vault_id).await.unwrap();

			let destination_public_key = PublicKey::from_binary(issue.vault_stellar_public_key);
			let stroop_amount = primitives::BalanceConversion::lookup(issue.amount + issue.fee)
				.expect("Invalid amount");
			let stellar_asset =
				primitives::AssetConversion::lookup(issue.asset).expect("Asset not found");

			let mut wallet_write = wallet.write().await;
			let result = wallet_write
				.send_payment_to_address(
					destination_public_key,
					stellar_asset,
					stroop_amount,
					issue.issue_id.0,
					300,
				)
				.await;
			assert!(result.is_ok());
			drop(wallet_write);

			tracing::warn!("Sent payment to address. Ledger is {:?}", result.unwrap().ledger);

			// Sleep 3 seconds to give other thread some time to receive the RequestIssue event and
			// add it to the set
			sleep(Duration::from_secs(3)).await;

			// wait for vault2 to execute this issue
			assert_event::<ExecuteIssueEvent, _>(TIMEOUT, user_provider.clone(), move |x| {
				x.vault_id == vault_id.clone() && x.amount == issue_amount
			})
			.await;
		};

		let wallet_read = wallet.read().await;
		let issue_filter = IssueFilter::new(&wallet_read.get_public_key()).expect("Invalid filter");
		let slot_tx_env_map = Arc::new(RwLock::new(HashMap::new()));

		let issue_set = Arc::new(RwLock::new(IssueRequestsMap::new()));
		let memos_to_issue_ids = Arc::new(RwLock::new(IssueIdLookup::new()));
		let (issue_event_tx, _issue_event_rx) = mpsc::channel::<CancellationEvent>(16);
		let service = join3(
			vault::service::listen_for_new_transactions(
				wallet_read.get_public_key(),
				wallet_read.is_public_network(),
				slot_tx_env_map.clone(),
				issue_set.clone(),
				memos_to_issue_ids.clone(),
				issue_filter,
			),
			vault::service::listen_for_issue_requests(
				vault_provider.clone(),
				wallet_read.get_public_key(),
				issue_event_tx,
				issue_set.clone(),
				memos_to_issue_ids.clone(),
			),
			vault::service::process_issues_requests(
				vault_provider.clone(),
				oracle_agent.clone(),
				slot_tx_env_map.clone(),
				issue_set.clone(),
				memos_to_issue_ids.clone(),
			),
		);
		drop(wallet_read);

		test_service(service, fut_user).await;
	})
	.await;
}

/// This test demonstrates that a vault can execute an issue request even if it is not the original
/// vault that was requested to execute the issue. This is possible because anyone can execute an
/// issue request. (However, only the requester of the issue can execute payments with insufficient
/// amounts).
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_automatic_issue_execution_succeeds_for_other_vault() {
	test_with_vault(|client, wallet, oracle_agent, vault1_id, vault1_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;
		let vault2_provider = setup_provider(client.clone(), AccountKeyring::Eve).await;
		let vault2_id = VaultId::new(
			AccountKeyring::Eve.into(),
			DEFAULT_TESTING_CURRENCY,
			DEFAULT_WRAPPED_CURRENCY,
		);

		let issue_amount = upscaled_compatible_amount(100);

		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault1_provider,
			issue_amount,
			vault1_id.wrapped_currency(),
			vault1_id.collateral_currency(),
		)
		.await;

		let wallet_read = wallet.read().await;
		assert_ok!(
			vault1_provider
				.register_vault_with_public_key(
					&vault1_id,
					vault_collateral,
					wallet_read.get_public_key_raw(),
				)
				.await
		);
		assert_ok!(
			vault2_provider
				.register_vault_with_public_key(
					&vault2_id,
					vault_collateral,
					wallet_read.get_public_key_raw(),
				)
				.await
		);
		drop(wallet_read);

		let issue_set_arc = Arc::new(RwLock::new(IssueRequestsMap::new()));
		let memos_to_issue_ids = Arc::new(RwLock::new(IssueIdLookup::new()));

		let slot_tx_env_map = Arc::new(RwLock::new(HashMap::new()));

		let fut_user = async {
			// The account of the 'user_provider' is used to request a new issue that
			// has to be executed by vault1
			let issue = user_provider.request_issue(issue_amount, &vault1_id).await.unwrap();

			let destination_public_key = PublicKey::from_binary(issue.vault_stellar_public_key);
			let stroop_amount = primitives::BalanceConversion::lookup(issue.amount + issue.fee)
				.expect("Invalid amount");
			let stellar_asset =
				primitives::AssetConversion::lookup(issue.asset).expect("Asset not found");

			// Sleep 1 second to give other thread some time to receive the RequestIssue event and
			// add it to the set
			sleep(Duration::from_secs(1)).await;
			let issue_set = issue_set_arc.read().await;
			assert!(!issue_set.is_empty());
			drop(issue_set);
			assert!(!memos_to_issue_ids.read().await.is_empty());

			let mut wallet_write = wallet.write().await;
			let result = wallet_write
				.send_payment_to_address(
					destination_public_key,
					stellar_asset,
					stroop_amount,
					issue.issue_id.0,
					300,
				)
				.await;
			assert!(result.is_ok());
			drop(wallet_write);

			tracing::info!("Sent payment to address. Ledger is {:?}", result.unwrap().ledger);

			// Sleep 3 seconds to give other thread some time to receive the RequestIssue event and
			// add it to the set
			sleep(Duration::from_secs(3)).await;

			// wait for vault2 to execute this issue
			assert_event::<ExecuteIssueEvent, _>(TIMEOUT * 3, user_provider.clone(), move |x| {
				x.vault_id == vault1_id.clone()
			})
			.await;

			// wait a second to give the `listen_for_executed_issues()` service time to update the
			// issue set
			sleep(Duration::from_secs(1)).await;
			let issue_set = issue_set_arc.read().await;
			assert!(issue_set.is_empty());
			drop(issue_set);
			assert!(memos_to_issue_ids.read().await.is_empty());
		};

		let wallet_read = wallet.read().await;
		let vault_account_public_key = wallet_read.get_public_key();
		drop(wallet_read);
		let issue_filter = IssueFilter::new(&vault_account_public_key).expect("Invalid filter");

		let (issue_event_tx, _issue_event_rx) = mpsc::channel::<CancellationEvent>(16);
		let service = join4(
			vault::service::listen_for_new_transactions(
				vault_account_public_key.clone(),
				CFG.is_public_network(),
				slot_tx_env_map.clone(),
				issue_set_arc.clone(),
				memos_to_issue_ids.clone(),
				issue_filter,
			),
			vault::service::listen_for_issue_requests(
				vault2_provider.clone(),
				vault_account_public_key,
				issue_event_tx,
				issue_set_arc.clone(),
				memos_to_issue_ids.clone(),
			),
			vault::service::process_issues_requests(
				vault2_provider.clone(),
				oracle_agent.clone(),
				slot_tx_env_map.clone(),
				issue_set_arc.clone(),
				memos_to_issue_ids.clone(),
			),
			vault::service::listen_for_executed_issues(
				vault2_provider.clone(),
				issue_set_arc.clone(),
				memos_to_issue_ids.clone(),
			),
		);

		test_service(service, fut_user).await;
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_execute_open_requests_succeeds() {
	test_with_vault(|client, wallet, oracle_agent, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let vault_ids = vec![vault_id.clone()];
		let vault_id_manager =
			VaultIdManager::from_map(vault_provider.clone(), wallet.clone(), vault_ids);

		// We issue 1 (spacewalk-chain) unit
		let issue_amount = CurrencyId::Native.one();
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount,
			vault_id.wrapped_currency(),
			vault_id.collateral_currency(),
		)
		.await;

		let wallet_read = wallet.read().await;
		assert_ok!(
			vault_provider
				.register_vault_with_public_key(
					&vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw(),
				)
				.await
		);
		drop(wallet_read);

		assert_issue(&user_provider, wallet.clone(), &vault_id, issue_amount, oracle_agent.clone())
			.await;

		let wallet_read = wallet.read().await;
		let address = wallet_read.get_public_key();
		let address_raw = wallet_read.get_public_key_raw();
		drop(wallet_read);
		// Place redeem requests. 100_00000 is our minimum redeem amount with the current fee
		// settings defined in the chain spec
		let redeem_ids = futures::future::join_all((0..3u128).map(|_| {
			user_provider.request_redeem(upscaled_compatible_amount(100), address_raw, &vault_id)
		}))
		.await
		.into_iter()
		.map(|x| x.unwrap())
		.collect::<Vec<_>>();

		let redeems: Vec<SpacewalkRedeemRequest> = futures::future::join_all(
			redeem_ids.iter().map(|id| user_provider.get_redeem_request(*id)),
		)
		.await
		.into_iter()
		.map(|x| x.unwrap())
		.collect::<Vec<_>>();

		let stroop_amount =
			primitives::BalanceConversion::lookup(redeems[0].amount).expect("Invalid amount");
		let asset = primitives::AssetConversion::lookup(redeems[0].asset).expect("Invalid asset");

		// do stellar transfer for redeem 0
		let mut wallet_write = wallet.write().await;
		assert_ok!(
			wallet_write
				.send_payment_to_address(address, asset, stroop_amount, redeem_ids[0].0, 300)
				.await
		);
		drop(wallet_write);

		// Sleep 3 seconds to give other thread some time to receive the RequestIssue event and
		// add it to the set
		sleep(Duration::from_secs(3)).await;

		let shutdown_tx = ShutdownSender::new();
		join4(
			vault::service::execute_open_requests(
				shutdown_tx.clone(),
				vault_provider,
				vault_id_manager,
				wallet.clone(),
				oracle_agent.clone(),
				Duration::from_secs(0),
			)
			.map(Result::unwrap),
			// Redeem 0 should be executed without creating an extra payment since we already sent
			// one just before
			assert_execute_redeem_event(TIMEOUT, user_provider.clone(), redeem_ids[0]),
			// Redeem 1 and 2 should be executed after creating an extra payment
			assert_execute_redeem_event(TIMEOUT, user_provider.clone(), redeem_ids[1]),
			assert_execute_redeem_event(TIMEOUT, user_provider.clone(), redeem_ids[2]),
		)
		.await;
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_off_chain_liquidation() {
	test_with_vault(|client, wallet, oracle_agent, vault_id, vault_provider| async move {
		// Bob is set as an authorized oracle in the chain_spec
		let authorized_oracle_provider = setup_provider(client.clone(), AccountKeyring::Bob).await;
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let issue_amount = upscaled_compatible_amount(100);
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount,
			vault_id.wrapped_currency(),
			vault_id.collateral_currency(),
		)
		.await;

		let wallet_read = wallet.read().await;
		assert_ok!(
			vault_provider
				.register_vault_with_public_key(
					&vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw()
				)
				.await
		);
		drop(wallet_read);

		assert_issue(&user_provider, wallet.clone(), &vault_id, issue_amount, oracle_agent.clone())
			.await;

		// Reduce price of testing currency from 1:1 to 100:1 to trigger liquidation
		set_exchange_rate_and_wait(
			&authorized_oracle_provider,
			DEFAULT_TESTING_CURRENCY,
			FixedU128::saturating_from_rational(1, 100),
		)
		.await;

		assert_event::<LiquidateVaultEvent, _>(TIMEOUT, vault_provider.clone(), |_| true).await;
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_shutdown() {
	test_with(|client, wallet| async move {
		let sudo_provider = setup_provider(client.clone(), AccountKeyring::Alice).await;
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let sudo_vault_id = VaultId::new(
			AccountKeyring::Alice.into(),
			DEFAULT_TESTING_CURRENCY,
			DEFAULT_WRAPPED_CURRENCY,
		);

		// register a vault..
		let issue_amount = upscaled_compatible_amount(100);
		let vault_collateral = get_required_vault_collateral_for_issue(
			&sudo_provider,
			issue_amount,
			sudo_vault_id.wrapped_currency(),
			sudo_vault_id.collateral_currency(),
		)
		.await;

		assert_ok!(
			sudo_provider
				.register_vault_with_public_key(
					&sudo_vault_id,
					vault_collateral,
					wallet.read().await.get_public_key_raw(),
				)
				.await
		);

		// shutdown chain..
		assert_ok!(
			sudo_provider
				.sudo(EncodedCall::Security(SecurityCall::set_parachain_status {
					status_code: StatusCode::Shutdown,
				}))
				.await
		);

		// request issue should fail:
		let result = user_provider.request_issue(issue_amount, &sudo_vault_id).await.unwrap_err();
		assert!(result.is_parachain_shutdown_error());

		// restore parachain status and check that we can issue now
		assert_ok!(
			sudo_provider
				.sudo(EncodedCall::Security(SecurityCall::set_parachain_status {
					status_code: StatusCode::Running,
				}))
				.await
		);
		assert_ok!(user_provider.request_issue(issue_amount, &sudo_vault_id).await);
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_requests_with_incompatible_amounts_fail() {
	test_with_vault(|client, wallet, _, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		// We define an incompatible amount
		let incompatible_amount = upscaled_compatible_amount(100) + 1;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			incompatible_amount,
			vault_id.wrapped_currency(),
			vault_id.collateral_currency(),
		)
		.await;

		let wallet_read = wallet.read().await;
		let address = wallet_read.get_public_key_raw();
		assert_ok!(
			vault_provider
				.register_vault_with_public_key(
					&vault_id,
					vault_collateral,
					wallet_read.get_public_key_raw()
				)
				.await
		);
		drop(wallet_read);

		// We try to request an issue with an incompatible amount
		let result = user_provider.request_issue(incompatible_amount, &vault_id).await;
		assert!(result.is_err());
		let error = result.unwrap_err();
		assert!(error.is_module_err("Currency", "IncompatibleAmount"));

		// We try to request a redeem with an incompatible amount
		let result = user_provider.request_redeem(incompatible_amount, address, &vault_id).await;
		assert!(result.is_err());
		let error = result.unwrap_err();
		assert!(error.is_module_err("Currency", "IncompatibleAmount"));

		// We try to request a replace with an incompatible amount
		let result = vault_provider.request_replace(&vault_id, incompatible_amount).await;
		assert!(result.is_err());
		let error = result.unwrap_err();
		assert!(error.is_module_err("Currency", "IncompatibleAmount"));
	})
	.await;
}
