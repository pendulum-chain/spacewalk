use std::{collections::HashMap, convert::TryInto, sync::Arc, time::Duration};

use async_trait::async_trait;
use frame_support::assert_ok;
use futures::{
	channel::mpsc,
	future::{join, join3, join4, join5},
	Future, FutureExt, SinkExt,
};
use serial_test::serial;
use sp_keyring::AccountKeyring;
use sp_runtime::traits::StaticLookup;
use tokio::{sync::RwLock, time::sleep};

use primitives::{issue::IssueRequest, H256};
use runtime::{
	integration::*, types::*, CurrencyId::Token, Error, FixedPointNumber, FixedU128, IssuePallet,
	RedeemPallet, ShutdownSender, SpacewalkParachain, SudoPallet, UtilFuncs, VaultRegistryPallet,
};
use stellar_relay_lib::sdk::{Hash, PublicKey, SecretKey, XdrCodec};
use vault::{
	oracle::{create_handler, types::Slot, OracleProofOps, Proof, ProofExt, ProofStatus},
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
	proof_ops: Arc<RwLock<OracleProofOps>>,
) {
	let issue = parachain_rpc.request_issue(amount, vault_id).await.unwrap();

	let asset = primitives::AssetConversion::lookup(issue.asset).expect("Invalid asset");
	let stroop_amount = amount as i64;
	let memo_hash = issue.issue_id.0;

	let mut wallet = wallet.write().await;
	let destination_address = wallet.get_public_key();
	let (response, tx_env) = wallet
		.send_payment_to_address(destination_address, asset, stroop_amount, memo_hash, 300)
		.await
		.expect("Failed to send payment");

	let slot = response.ledger as u64;

	let ops_read = proof_ops.read().await;

	// Loop pending proofs until it is ready
	let mut proof: Option<Proof> = None;
	with_timeout(
		async {
			loop {
				let proof_status = ops_read.get_proof(slot).await.expect("Failed to get proof");

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
	let tx_envelope_xdr_encoded = tx_env.to_base64_xdr();
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
#[serial]
async fn test_redeem_succeeds() {
	test_with_vault(|client, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let is_public_network = false;
		let wallet = StellarWallet::from_secret_encoded(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			is_public_network,
		)
		.unwrap();
		let wallet_arc = Arc::new(RwLock::new(wallet));

		let vault_ids = vec![vault_id.clone()];
		let vault_id_manager =
			VaultIdManager::from_map(vault_provider.clone(), wallet_arc.clone(), vault_ids);

		let issue_amount = 100000;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount,
			vault_id.collateral_currency(),
		)
		.await;
		tracing::error!("vault_collateral: {vault_collateral}");

		let wallet = wallet_arc.read().await;
		assert_ok!(
			vault_provider
				.register_vault_with_public_key(
					&vault_id,
					vault_collateral,
					wallet.get_public_key_raw()
				)
				.await
		);

		let shutdown_tx = ShutdownSender::new();

		// Setup scp handler for proofs
		// TODO refactor once improved 'OracleAgent' is implemented
		let handler = vault::inner_create_handler(wallet.get_secret_key(), is_public_network)
			.await
			.expect("Failed to create handler");
		let proof_ops = Arc::new(RwLock::new(handler.proof_operations()));
		drop(wallet);

		assert_issue(
			&user_provider,
			wallet_arc.clone(),
			&vault_id,
			issue_amount,
			proof_ops.clone(),
		)
		.await;

		test_service(
			vault::service::listen_for_redeem_requests(
				shutdown_tx,
				vault_provider.clone(),
				vault_id_manager,
				Duration::from_secs(0),
				proof_ops,
			),
			async {
				let wallet = wallet_arc.read().await;
				let address = wallet.get_public_key_raw();
				drop(wallet);
				let redeem_id =
					user_provider.request_redeem(10000, address, &vault_id).await.unwrap();
				assert_execute_redeem_event(TIMEOUT, user_provider, redeem_id).await;
			},
		)
		.await;
	})
	.await;
}

/*
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_cancel_scheduler_succeeds() {
	// tests cancellation of issue, redeem and replace.
	// issue and replace cancellation is tested through the vault's cancellation service.
	// cancel_redeem is called manually
	test_with_vault(|client, old_vault_id, old_vault_provider| async move {
		let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;

		let root_provider = setup_provider(client.clone(), AccountKeyring::Alice).await;
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;
		let new_vault_provider = setup_provider(client.clone(), AccountKeyring::Eve).await;
		// This conversion is necessary for now because subxt uses newer versions of the sp_xxx
		// dependencies
		let eve_account =
			subxt::ext::sp_runtime::AccountId32::from(AccountKeyring::Eve.to_raw_public());
		let new_vault_id =
			VaultId::new(eve_account, DEFAULT_TESTING_CURRENCY, DEFAULT_WRAPPED_CURRENCY);

		let is_public_network = false;
		let wallet = StellarWallet::from_secret_encoded(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			is_public_network,
		)
		.unwrap();
		let wallet = Arc::new(wallet);

		let issue_amount = 100000;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&old_vault_provider,
			issue_amount * 10,
			old_vault_id.collateral_currency(),
		)
		.await;
		assert_ok!(
			old_vault_provider
				.register_vault_with_public_key(
					&old_vault_id,
					vault_collateral,
					wallet.get_public_key_raw()
				)
				.await
		);
		assert_ok!(
			new_vault_provider
				.register_vault_with_public_key(
					&new_vault_id,
					vault_collateral,
					wallet.get_public_key_raw()
				)
				.await
		);

		// TODO why is this needed?
		// assert_issue(&user_provider, &parachain_rpc, &old_vault_id, issue_amount).await;

		// set low timeout periods
		assert_ok!(root_provider.set_issue_period(1).await);
		// assert_ok!(root_provider.set_replace_period(1).await);
		// assert_ok!(root_provider.set_redeem_period(1).await);

		let (issue_cancellation_event_tx, issue_cancellation_rx) =
			mpsc::channel::<CancellationEvent>(16);
		let (replace_cancellation_event_tx, replace_cancellation_rx) =
			mpsc::channel::<CancellationEvent>(16);

		let block_listener = new_vault_provider.clone();
		let issue_set = Arc::new(RwLock::new(IssueRequestsMap::new()));

		let issue_request_listener = vault::service::listen_for_issue_requests(
			new_vault_provider.clone(),
			wallet.get_public_key(),
			issue_cancellation_event_tx.clone(),
			issue_set.clone(),
		);

		let issue_cancellation_scheduler = vault::service::CancellationScheduler::new(
			new_vault_provider.clone(),
			new_vault_provider.get_current_chain_height().await.unwrap(),
			new_vault_provider.get_account_id().clone(),
		);
		// let replace_cancellation_scheduler = vault::service::CancellationScheduler::new(
		// 	new_vault_provider.clone(),
		// 	new_vault_provider.get_current_chain_height().await.unwrap(),
		// 	0,
		// 	new_vault_provider.get_account_id().clone(),
		// );
		let issue_canceller = issue_cancellation_scheduler
			.handle_cancellation::<vault::service::IssueCanceller>(issue_cancellation_rx);
		// let replace_canceller = replace_cancellation_scheduler
		// 	.handle_cancellation::<vault::service::ReplaceCanceller>(replace_cancellation_rx);

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
			join3(
				issue_canceller.map(Result::unwrap),
				// replace_canceller.map(Result::unwrap),
				issue_request_listener.map(Result::unwrap),
				parachain_block_listener,
			),
			async {
				// let address = BtcAddress::P2PKH(H160::from_slice(&[2; 20]));

				// setup the to-be-cancelled redeem
				// let redeem_id =
				// 	user_provider.request_redeem(20000, address, &old_vault_id).await.unwrap();

				join(
					async {
						// setup the to-be-cancelled replace
						// assert_ok!(
						// 	old_vault_provider
						// 		.request_replace(&old_vault_id, issue_amount / 2)
						// 		.await
						// );
						// assert_ok!(
						// 	new_vault_provider
						// 		.accept_replace(
						// 			&new_vault_id,
						// 			&old_vault_id,
						// 			10000000u32.into(),
						// 			0u32.into(),
						// 			address
						// 		)
						// 		.await
						// );
						// assert_ok!(
						// 	replace_cancellation_event_tx
						// 		.clone()
						// 		.send(CancellationEvent::Opened)
						// 		.await
						// );
						//
						// setup the to-be-cancelled issue
						assert_ok!(user_provider.request_issue(issue_amount, &new_vault_id).await);

						// todo: the `send_to_address` is still a todo.
						// for _ in 0u32..2 {
						// 	assert_ok!(
						// 		parachain_rpc
						// 			.send_to_address(
						// 				BtcAddress::P2PKH(H160::from_slice(&[0; 20]))
						// 					.to_address(parachain_rpc.network())
						// 					.unwrap(),
						// 				100_000,
						// 				None,
						// 				SatPerVbyte(1000),
						// 				1
						// 			)
						// 			.await
						// 	);
						// }
					},
					assert_event::<CancelIssueEvent, _>(
						Duration::from_secs(120),
						user_provider.clone(),
						|_| true,
					),
					// assert_event::<CancelReplaceEvent, _>(
					// 	Duration::from_secs(120),
					// 	user_provider.clone(),
					// 	|_| true,
					// ),
				)
				.await;

				// now make sure we can cancel the redeem
				// assert_ok!(user_provider.cancel_redeem(redeem_id, true).await);
			},
		)
		.await;
	})
	.await;
}
*/

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_issue_cancel_succeeds() {
	test_with_vault(|client, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;
		let issue_set = Arc::new(RwLock::new(IssueRequestsMap::new()));

		let is_public_network = false;
		let mut wallet = StellarWallet::from_secret_encoded(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			is_public_network,
		)
		.unwrap();
		let issue_filter = IssueFilter::new(&wallet.get_public_key()).expect("Invalid filter");

		let issue_amount = 100000;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount,
			vault_id.collateral_currency(),
		)
		.await;

		assert_ok!(
			vault_provider
				.register_vault_with_public_key(
					&vault_id,
					vault_collateral,
					wallet.get_public_key_raw(),
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
		};

		let slot_tx_env_map: Arc<RwLock<HashMap<u32, String>>> =
			Arc::new(RwLock::new(HashMap::new()));

		let handler = vault::inner_create_handler(wallet.get_secret_key(), is_public_network)
			.await
			.expect("Failed to create handler");
		let watcher = Arc::new(RwLock::new(handler.create_watcher()));

		let (issue_event_tx, _issue_event_rx) = mpsc::channel::<CancellationEvent>(16);
		let service = join3(
			vault::service::listen_for_new_transactions(
				wallet.get_public_key(),
				wallet.is_public_network(),
				watcher.clone(),
				slot_tx_env_map.clone(),
				issue_set.clone(),
				issue_filter,
			),
			vault::service::listen_for_issue_requests(
				vault_provider.clone(),
				wallet.get_public_key(),
				issue_event_tx,
				issue_set.clone(),
			),
			vault::service::listen_for_issue_cancels(vault_provider.clone(), issue_set.clone()),
		);

		test_service(service, fut_user).await;
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_issue_overpayment_succeeds() {
	test_with_vault(|client, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let is_public_network = false;
		let mut wallet = StellarWallet::from_secret_encoded(
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
				300,
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
#[serial]
async fn test_automatic_issue_execution_succeeds() {
	test_with_vault(|client, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let is_public_network = false;
		let wallet = StellarWallet::from_secret_encoded(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			is_public_network,
		)
		.unwrap();
		let wallet_arc = Arc::new(RwLock::new(wallet));

		let issue_amount = 100000;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount,
			vault_id.collateral_currency(),
		)
		.await;

		let wallet = wallet_arc.read().await;
		assert_ok!(
			vault_provider
				.register_vault_with_public_key(
					&vault_id,
					vault_collateral,
					wallet.get_public_key_raw(),
				)
				.await
		);
		drop(wallet);

		let fut_user = async {
			// The account of the 'user_provider' is used to request a new issue that
			// has to be executed by the vault_provider
			let issue = user_provider.request_issue(issue_amount, &vault_id).await.unwrap();

			let destination_public_key = PublicKey::from_binary(issue.vault_stellar_public_key);
			let stroop_amount = (issue.amount + issue.fee) as i64;
			let stellar_asset =
				primitives::AssetConversion::lookup(issue.asset).expect("Asset not found");
			let memo_hash = issue.issue_id.0;

			let mut wallet = wallet_arc.write().await;
			let result = wallet
				.send_payment_to_address(
					destination_public_key,
					stellar_asset,
					stroop_amount,
					memo_hash,
					300,
				)
				.await;
			assert!(result.is_ok());
			drop(wallet);

			tracing::warn!("Sent payment to address. Ledger is {:?}", result.unwrap().0.ledger);

			// wait for vault2 to execute this issue
			assert_event::<ExecuteIssueEvent, _>(TIMEOUT, user_provider.clone(), move |x| {
				x.vault_id == vault_id.clone()
			})
			.await;
		};

		let slot_tx_env_map: Arc<RwLock<HashMap<u32, String>>> =
			Arc::new(RwLock::new(HashMap::new()));

		let wallet = wallet_arc.read().await;
		let issue_filter = IssueFilter::new(&wallet.get_public_key()).expect("Invalid filter");
		let handler = vault::inner_create_handler(wallet.get_secret_key(), is_public_network)
			.await
			.expect("Failed to create handler");
		let watcher = Arc::new(RwLock::new(handler.create_watcher()));
		let proof_ops = Arc::new(RwLock::new(handler.proof_operations()));

		let issue_set = Arc::new(RwLock::new(IssueRequestsMap::new()));
		let (issue_event_tx, _issue_event_rx) = mpsc::channel::<CancellationEvent>(16);
		let service = join3(
			vault::service::listen_for_new_transactions(
				wallet.get_public_key(),
				wallet.is_public_network(),
				watcher.clone(),
				slot_tx_env_map.clone(),
				issue_set.clone(),
				issue_filter,
			),
			vault::service::listen_for_issue_requests(
				vault_provider.clone(),
				wallet.get_public_key(),
				issue_event_tx,
				issue_set.clone(),
			),
			vault::service::process_issues_with_proofs(
				vault_provider.clone(),
				proof_ops.clone(),
				slot_tx_env_map.clone(),
				issue_set.clone(),
			),
		);
		drop(wallet);

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
	test_with_vault(|client, vault1_id, vault1_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;
		let vault2_provider = setup_provider(client.clone(), AccountKeyring::Eve).await;

		// This conversion is necessary for now because subxt uses newer versions of the sp_xxx
		// dependencies
		let eve_account =
			subxt::ext::sp_runtime::AccountId32::from(AccountKeyring::Eve.to_raw_public());
		let vault2_id =
			VaultId::new(eve_account, DEFAULT_TESTING_CURRENCY, DEFAULT_WRAPPED_CURRENCY);

		let is_public_network = false;
		let wallet = StellarWallet::from_secret_encoded(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			is_public_network,
		)
		.unwrap();
		let wallet_arc = Arc::new(RwLock::new(wallet));

		let issue_amount = 100000;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault1_provider,
			issue_amount,
			vault1_id.collateral_currency(),
		)
		.await;

		let wallet = wallet_arc.read().await;
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
		drop(wallet);

		let issue_set = Arc::new(RwLock::new(IssueRequestsMap::new()));
		let slot_tx_env_map: Arc<RwLock<HashMap<u32, String>>> =
			Arc::new(RwLock::new(HashMap::new()));

		let fut_user = async {
			// The account of the 'user_provider' is used to request a new issue that
			// has to be executed by vault1
			let issue = user_provider.request_issue(issue_amount, &vault1_id).await.unwrap();

			let destination_public_key = PublicKey::from_binary(issue.vault_stellar_public_key);
			let stroop_amount = (issue.amount + issue.fee) as i64;
			let stellar_asset =
				primitives::AssetConversion::lookup(issue.asset).expect("Asset not found");
			let memo_hash = issue.issue_id.0;

			// Wrap this in a block to make sure the lock is dropped
			{
				let issue_set = issue_set.read().await;
				assert!(!issue_set.is_empty());
			}

			let mut wallet = wallet_arc.write().await;
			let result = wallet
				.send_payment_to_address(
					destination_public_key,
					stellar_asset,
					stroop_amount,
					memo_hash,
					300,
				)
				.await;
			assert!(result.is_ok());
			drop(wallet);

			tracing::info!("Sent payment to address. Ledger is {:?}", result.unwrap().0.ledger);

			// wait for vault2 to execute this issue
			assert_event::<ExecuteIssueEvent, _>(TIMEOUT, user_provider.clone(), move |x| {
				x.vault_id == vault1_id.clone()
			})
			.await;

			// Wrap this in a block to make sure the lock is dropped
			{
				let issue_set = issue_set.read().await;
				assert!(issue_set.is_empty());
			}
		};

		let wallet = wallet_arc.read().await;
		let issue_filter = IssueFilter::new(&wallet.get_public_key()).expect("Invalid filter");
		let handler = vault::inner_create_handler(wallet.get_secret_key(), is_public_network)
			.await
			.expect("Failed to create handler");
		let watcher = Arc::new(RwLock::new(handler.create_watcher()));
		let proof_ops = Arc::new(RwLock::new(handler.proof_operations()));

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
				issue_event_tx,
				issue_set.clone(),
			),
			vault::service::process_issues_with_proofs(
				vault2_provider.clone(),
				proof_ops.clone(),
				slot_tx_env_map.clone(),
				issue_set.clone(),
			),
			vault::service::listen_for_executed_issues(vault2_provider.clone(), issue_set.clone()),
		);

		test_service(service, fut_user).await;
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_execute_open_requests_succeeds() {
	test_with_vault(|client, vault_id, vault_provider| async move {
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let is_public_network = false;
		let wallet = StellarWallet::from_secret_encoded(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			is_public_network,
		)
		.unwrap();
		let wallet_arc = Arc::new(RwLock::new(wallet));

		let vault_ids = vec![vault_id.clone()];
		let vault_id_manager =
			VaultIdManager::from_map(vault_provider.clone(), wallet_arc.clone(), vault_ids);

		let issue_amount = 100000;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount,
			vault_id.collateral_currency(),
		)
		.await;

		let wallet = wallet_arc.read().await;
		assert_ok!(
			vault_provider
				.register_vault_with_public_key(
					&vault_id,
					vault_collateral,
					wallet.get_public_key_raw(),
				)
				.await
		);

		// Setup scp handler for proofs
		// TODO refactor once improved 'OracleAgent' is implemented
		let handler = vault::inner_create_handler(wallet.get_secret_key(), is_public_network)
			.await
			.expect("Failed to create handler");
		let proof_ops = Arc::new(RwLock::new(handler.proof_operations()));

		drop(wallet);

		assert_issue(
			&user_provider,
			wallet_arc.clone(),
			&vault_id,
			issue_amount,
			proof_ops.clone(),
		)
		.await;

		let mut wallet = wallet_arc.write().await;
		let address = wallet.get_public_key();
		let address_raw = wallet.get_public_key_raw();
		// place replace requests
		let redeem_ids = futures::future::join_all(
			(0..3u128).map(|_| user_provider.request_redeem(10000, address_raw, &vault_id)),
		)
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

		let stroop_amount = redeems[0].amount as i64;
		let asset = primitives::AssetConversion::lookup(redeems[0].asset).expect("Invalid asset");
		let memo_hash = redeem_ids[0].0;
		// do stellar transfer for redeem 0
		assert_ok!(
			wallet
				.send_payment_to_address(address, asset, stroop_amount, memo_hash, 300)
				.await
		);

		drop(wallet);

		let shutdown_tx = ShutdownSender::new();
		join4(
			vault::service::execute_open_requests(
				shutdown_tx.clone(),
				vault_provider,
				vault_id_manager,
				wallet_arc.clone(),
				proof_ops,
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
async fn test_shutdown() {
	test_with(|client| async move {
		let sudo_provider = setup_provider(client.clone(), AccountKeyring::Alice).await;
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		// This conversion is necessary for now because subxt uses newer versions of the sp_xxx
		// dependencies
		let alice_account =
			subxt::ext::sp_runtime::AccountId32::from(AccountKeyring::Alice.to_raw_public());
		let sudo_vault_id =
			VaultId::new(alice_account, DEFAULT_TESTING_CURRENCY, DEFAULT_WRAPPED_CURRENCY);

		let is_public_network = false;
		let wallet = StellarWallet::from_secret_encoded(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			is_public_network,
		)
		.unwrap();

		// register a vault..
		let issue_amount = 100000;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&sudo_provider,
			issue_amount,
			sudo_vault_id.collateral_currency(),
		)
		.await;

		assert_ok!(
			sudo_provider
				.register_vault_with_public_key(
					&sudo_vault_id,
					vault_collateral,
					wallet.get_public_key_raw(),
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
