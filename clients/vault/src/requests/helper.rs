use async_trait::async_trait;
use futures::try_join;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;

use crate::{requests::structs::Request, Error, VaultIdManager};

use crate::oracle::OracleAgent;
use primitives::{derive_shortened_request_id, TextMemo, TransactionEnvelopeExt};
use runtime::{
	AccountId, RedeemPallet, RedeemRequestStatus, ReplacePallet, ReplaceRequestStatus,
	ShutdownSender, SpacewalkParachain,
};
use service::Error as ServiceError;
use wallet::{StellarWallet, TransactionResponse, TransactionsResponseIter};

#[async_trait]
pub(crate) trait PayAndExecuteExt<R> {
	/// Spawns cancelable task for each open request.
	/// The task performs payment and execution of the open request.
	///
	///  # Arguments
	///
	/// * `requests` - open/pending requests that requires Stellar payment before execution
	/// * `vault_id_manager` - contains all the vault ids and their data
	/// * `shutdown_tx` - for sending and receiving shutdown signals
	/// * `parachain_rpc` - the parachain RPC handle
	/// * `oracle_agent` - the agent used to get the proofs
	/// * `rate_limiter` - rate limiter
	fn spawn_tasks_to_pay_and_execute_open_requests(
		requests: HashMap<TextMemo, Request>,
		vault_id_manager: VaultIdManager,
		shutdown_tx: ShutdownSender,
		parachain_rpc: &SpacewalkParachain,
		oracle_agent: Arc<OracleAgent>,
		rate_limiter: Arc<R>,
	);

	/// Performs payment and execution of the open request.
	/// The stellar address of the open request receives the payment; and
	/// the vault id of the open request sends the payment.
	/// However, the vault id MUST exist in the vault_id_manager.
	///
	///  # Arguments
	///
	/// * `request` - the open request
	/// * `vault_id_manager` - contains all the vault ids and their data.
	/// * `parachain_rpc` - the parachain RPC handle
	/// * `oracle_agent` - the agent used to get the proofs
	/// * `rate_limiter` - rate limiter
	async fn pay_and_execute_open_request_async(
		request: Request,
		vault_id_manager: VaultIdManager,
		parachain_rpc: SpacewalkParachain,
		oracle_agent: Arc<OracleAgent>,
		rate_limiter: Arc<R>,
	);
}

/// Returns an iter of all known transactions of the wallet
pub(crate) async fn get_all_transactions_of_wallet_async(
	wallet: Arc<RwLock<StellarWallet>>,
) -> Option<TransactionsResponseIter> {
	// Queries all known transactions for the targeted vault account and check if any of
	// them is targeted.
	let wallet = wallet.read().await;
	let transactions_result = wallet.get_all_transactions_iter().await;
	drop(wallet);

	match transactions_result {
		Err(e) => {
			tracing::error!("Failed to get transactions from Stellar: {e}");
			None
		},
		Ok(transactions) => Some(transactions),
	}
}

/// Get the Request from the hashmap that the given Transaction satisfies, based
/// on the amount of assets that is transferred to the address.
pub(crate) fn get_request_for_stellar_tx(
	tx: &TransactionResponse,
	hash_map: &HashMap<TextMemo, Request>,
) -> Option<Request> {
	let memo_text = tx.memo_text()?;
	let request = hash_map.get(memo_text)?;

	let envelope = tx.to_envelope().ok()?;
	let paid_amount =
		envelope.get_payment_amount_for_asset_to(request.stellar_address(), request.asset());

	if paid_amount >= request.amount() {
		return Some(request.clone())
	}

	None
}

/// Returns all open or "pending" `Replace` and `Redeem` requests
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `vault_id` - account ID of the vault
/// * `payment_margin` - minimum time to the the redeem execution deadline to make the stellar
///   payment.
pub(crate) async fn retrieve_open_redeem_replace_requests_async(
	parachain_rpc: &SpacewalkParachain,
	vault_id: AccountId,
	payment_margin: Duration,
) -> Result<HashMap<TextMemo, Request>, ServiceError<Error>> {
	// get all redeem and replace requests
	let (redeem_requests, replace_requests) = try_join!(
		parachain_rpc.get_vault_redeem_requests(vault_id.clone()),
		parachain_rpc.get_old_vault_replace_requests(vault_id),
	)?;

	let open_redeems = redeem_requests
		.into_iter()
		.filter(|(_, request)| request.status == RedeemRequestStatus::Pending)
		.filter_map(|(hash, request)| {
			Request::from_redeem_request(hash, request, payment_margin).ok()
		});

	let open_replaces = replace_requests
		.into_iter()
		.filter(|(_, request)| request.status == ReplaceRequestStatus::Pending)
		.filter_map(|(hash, request)| {
			Request::from_replace_request(hash, request, payment_margin).ok()
		});

	// collect all requests into a hashmap, indexed by their id
	Ok(open_redeems
		.chain(open_replaces)
		.map(|x| (derive_shortened_request_id(&x.hash_inner()), x))
		.collect::<HashMap<_, _>>())
}
