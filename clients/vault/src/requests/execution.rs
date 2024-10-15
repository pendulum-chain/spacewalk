use crate::{
	error::Error,
	oracle::OracleAgent,
	requests::{
		helper::{
			get_all_transactions_of_wallet_async, get_request_for_stellar_tx,
			retrieve_open_redeem_replace_requests_async, PayAndExecuteExt,
		},
		structs::Request,
		PayAndExecute,
	},
	ArcRwLock, VaultIdManager, YIELD_RATE,
};
use async_trait::async_trait;
use governor::{
	clock::{Clock, ReasonablyRealtime},
	middleware::RateLimitingMiddleware,
	state::{DirectStateStore, NotKeyed},
	NotUntil, RateLimiter,
};
use primitives::{derive_shortened_request_id, stellar::TransactionEnvelope, TextMemo};
use runtime::{PrettyPrint, ShutdownSender, SpacewalkParachain, UtilFuncs};
use service::{spawn_cancelable, Error as ServiceError};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use wallet::{Slot, StellarWallet, TransactionResponse};

// max of 3 retries for failed request execution
const MAX_EXECUTION_RETRIES: u32 = 3;

/// Spawns cancelable task for each open request.
/// The task performs the `execute` function of the request.
///
/// # Arguments
///
/// * `wallet` - the vault's wallet; used to retrieve a list of stellar transactions
/// * `requests` - a list of all open/pending requests
/// * `shutdown_tx` - for sending and receiving shutdown signals
/// * `parachain_rpc` - the parachain RPC handle
/// * `oracle_agent` - the agent used to get the proofs
/// * `rate_limiter` - a rate limiter
async fn spawn_tasks_to_execute_open_requests_async<S, C, MW>(
	requests: &mut HashMap<TextMemo, Request>,
	wallet: ArcRwLock<StellarWallet>,
	shutdown_tx: ShutdownSender,
	parachain_rpc: &SpacewalkParachain,
	oracle_agent: Arc<OracleAgent>,
	rate_limiter: Arc<RateLimiter<NotKeyed, S, C, MW>>,
) where
	S: DirectStateStore,
	C: ReasonablyRealtime,
	MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>,
{
	if let Some(mut tx_iter) = get_all_transactions_of_wallet_async(wallet).await {
		// Check if some of the open requests have a corresponding payment on Stellar
		// and are just waiting to be executed on the parachain
		while let Some(transaction) = tx_iter.next().await {
			if rate_limiter.check().is_ok() {
				// give the outer `select` a chance to check the shutdown signal
				tokio::task::yield_now().await;
			}

			// stop the loop
			if requests.is_empty() {
				break;
			}

			if let Some(request) = get_request_for_stellar_tx(&transaction, &requests) {
				let hash_as_memo = spawn_task_to_execute_open_request(
					request,
					transaction,
					shutdown_tx.clone(),
					parachain_rpc.clone(),
					oracle_agent.clone(),
				);

				// remove request from the hashmap, using the memo
				requests.retain(|key, _| key != &hash_as_memo);
			}
		}
	}
}

/// Spawns a cancelable task to execute an open request.
/// Returns the memo of the request.
///
/// # Arguments
///
/// * `request` - the open/pending request
/// * `transaction` - the transaction that the request is based from
/// * `shutdown_tx` - for sending and receiving shutdown signals
/// * `parachain_rpc` - the parachain RPC handle
/// * `oracle_agent` - the agent used to get the proofs
fn spawn_task_to_execute_open_request(
	request: Request,
	transaction: TransactionResponse,
	shutdown_tx: ShutdownSender,
	parachain_rpc: SpacewalkParachain,
	oracle_agent: Arc<OracleAgent>,
) -> TextMemo {
	let hash_as_memo = derive_shortened_request_id(&request.hash_inner());

	tracing::info!(
		"Processing valid Stellar payment for open {:?} request #{}: ",
		request.request_type(),
		request.hash()
	);

	match transaction.to_envelope() {
		Err(e) => {
			tracing::error!(
				"Failed to decode transaction envelope for {:?} request #{}: {e:?}",
				request.request_type(),
				request.hash()
			);
		},
		Ok(tx_envelope) => {
			// start a new task to execute on the parachain
			spawn_cancelable(
				shutdown_tx.subscribe(),
				execute_open_request_async(
					request,
					tx_envelope,
					transaction.ledger as Slot,
					parachain_rpc,
					oracle_agent,
				),
			);
		},
	}

	hash_as_memo
}

/// Executes the open request based on the transaction envelope and the proof.
/// The proof is obtained using the slot.
///
///  # Arguments
///
/// * `request` - the open request
/// * `tx_envelope` - the transaction envelope that the request is based from
/// * `slot` - the ledger number of the transaction envelope
/// * `parachain_rpc` - the parachain RPC handle
/// * `oracle_agent` - the agent used to get the proofs
async fn execute_open_request_async(
	request: Request,
	tx_envelope: TransactionEnvelope,
	slot: Slot,
	parachain_rpc: SpacewalkParachain,
	oracle_agent: Arc<OracleAgent>,
) {
	let mut retry_count = 0; // A counter for every execution retry

	while retry_count < MAX_EXECUTION_RETRIES {
		if retry_count > 0 {
			tracing::info!("Performing retry #{retry_count} out of {MAX_EXECUTION_RETRIES} retries for {:?} request #{}",request.request_type(),request.hash());
		}

		match oracle_agent.get_proof(slot).await {
			Ok(proof) => {
				let Err(e) =
					request.execute(parachain_rpc.clone(), tx_envelope.clone(), proof).await
				else {
					tracing::info!(
						"Successfully executed {:?} request #{}",
						request.request_type(),
						request.hash()
					);

					break; // There is no need to retry again, so exit from while loop
				};

				tracing::error!(
					"Failed to execute {:?} request #{} because of error: {e:?}",
					request.request_type(),
					request.hash()
				);
				break; // There is also no need to retry on an unrecoverable error.
			},
			Err(error) => {
				retry_count += 1; // increase retry count
				tracing::error!("Failed to get proof for slot {slot} for {:?} request #{:?} due to error: {error:?}",
                    request.request_type(),
                    request.hash(),
                );
			},
		}
	}

	if retry_count >= MAX_EXECUTION_RETRIES {
		tracing::error!("Exceeded max number of retries ({MAX_EXECUTION_RETRIES}) to execute {:?} request #{:?}. Giving up...",
            request.request_type(),
            request.hash(),
        );
	}
}

#[async_trait]
impl<S, C, MW> PayAndExecuteExt<RateLimiter<NotKeyed, S, C, MW>> for PayAndExecute
where
	S: DirectStateStore + Send + Sync + 'static,
	C: ReasonablyRealtime + Send + Sync + 'static,
	MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>
		+ Send
		+ Sync
		+ 'static,
	<MW as RateLimitingMiddleware<<C as Clock>::Instant>>::PositiveOutcome: Send,
{
	fn spawn_tasks_to_pay_and_execute_open_requests(
		requests: HashMap<TextMemo, Request>,
		vault_id_manager: VaultIdManager,
		shutdown_tx: ShutdownSender,
		parachain_rpc: &SpacewalkParachain,
		oracle_agent: Arc<OracleAgent>,
		rate_limiter: Arc<RateLimiter<NotKeyed, S, C, MW>>,
	) {
		for (_, request) in requests {
			// there are potentially a large number of open requests - pay and execute each
			// in a separate task to ensure that awaiting confirmations does not significantly
			// delay other requests
			// make copies of the variables we move into the task
			spawn_cancelable(
				shutdown_tx.subscribe(),
				Self::pay_and_execute_open_request_async(
					request,
					vault_id_manager.clone(),
					parachain_rpc.clone(),
					oracle_agent.clone(),
					rate_limiter.clone(),
				),
			);
		}
	}

	async fn pay_and_execute_open_request_async(
		request: Request,
		vault_id_manager: VaultIdManager,
		parachain_rpc: SpacewalkParachain,
		oracle_agent: Arc<OracleAgent>,
		rate_limiter: Arc<RateLimiter<NotKeyed, S, C, MW>>,
	) {
		let Some(vault) = vault_id_manager.get_vault(request.vault_id()).await else {
			tracing::error!(
				"Couldn't process open {:?} request #{:?}: Failed to fetch vault data for vault {}",
				request.request_type(),
				request.hash(),
				request.vault_id().pretty_print()
			);

			return; // nothing we can do - bail
		};

		// We rate limit the number of transactions we pay and execute simultaneously because
		// sending too many at once might cause the Stellar network to respond with a timeout
		// error.
		rate_limiter.until_ready().await;

		match request.pay_and_execute(parachain_rpc, vault, oracle_agent).await {
			Ok(_) => tracing::info!(
				"Successfully executed open {:?} request #{:?}",
				request.request_type(),
				request.hash()
			),
			Err(e) => tracing::info!(
				"Failed to process open {:?} request #{:?} due to error: {e}",
				request.request_type(),
				request.hash(),
			),
		}
	}
}

/// Queries the parachain for open requests and executes them. It checks the
/// stellar blockchain to see if a payment has already been made.
///
///  # Arguments
///
/// * `shutdown_tx` - for sending and receiving shutdown signals
/// * `parachain_rpc` - the parachain RPC handle
/// * `vault_id_manager` - contains all the vault ids and their data.
/// * `wallet` - the vault's wallet; used to retrieve a list of stellar transactions
/// * `oracle_agent` - the agent used to get the proofs
/// * `payment_margin` - minimum time to the redeem execution deadline to make the stellar
/// * `precheck_signal` - a signal sender to notify the caller that this process is done
/// and pending tasks can be started
/// payment.
#[allow(clippy::too_many_arguments)]
pub async fn execute_open_requests(
	shutdown_tx: ShutdownSender,
	parachain_rpc: SpacewalkParachain,
	vault_id_manager: VaultIdManager,
	wallet: Arc<RwLock<StellarWallet>>,
	oracle_agent: Arc<OracleAgent>,
	payment_margin: Duration,
	precheck_signal: tokio::sync::broadcast::Sender<()>,
) -> Result<(), ServiceError<Error>> {
	tracing::info!("execute_open_requests(): started");
	let parachain_rpc_ref = &parachain_rpc;

	// get all redeem and replace requests
	let mut open_requests = retrieve_open_redeem_replace_requests_async(
		parachain_rpc_ref,
		parachain_rpc.get_account_id().clone(),
		payment_margin,
	)
	.await?;

	let rate_limiter = Arc::new(RateLimiter::direct(YIELD_RATE));

	tracing::info!("execute_open_requests(): Oracle agent is ready.");

	// Check if the open requests have a corresponding payment on Stellar
	// and are just waiting to be executed on the parachain
	spawn_tasks_to_execute_open_requests_async(
		&mut open_requests,
		wallet,
		shutdown_tx.clone(),
		parachain_rpc_ref,
		oracle_agent.clone(),
		rate_limiter.clone(),
	)
	.await;

	// Remaining requests in the hashmap did not have a Stellar payment yet,
	// so pay and execute all of these
	PayAndExecute::spawn_tasks_to_pay_and_execute_open_requests(
		open_requests,
		vault_id_manager,
		shutdown_tx,
		parachain_rpc_ref,
		oracle_agent,
		rate_limiter,
	);

	if let Err(e) = precheck_signal.send(()) {
		tracing::error!("execute_open_requests(): Failed to send signal: {e:?}");
	}

	Ok(())
}
