use futures::try_join;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;

use crate::{requests::structs::Request, Error};

use primitives::{derive_shortened_request_id, TextMemo, TransactionEnvelopeExt};
use runtime::{
	AccountId, RedeemPallet, RedeemRequestStatus, ReplacePallet, ReplaceRequestStatus,
	SpacewalkParachain,
};
use service::Error as ServiceError;
use wallet::{StellarWallet, TransactionResponse, TransactionsResponseIter};

/// Returns an iter of all known transactions of the wallet
pub(crate) async fn get_all_transactions_of_wallet_async(
	wallet: Arc<RwLock<StellarWallet>>,
) -> Option<TransactionsResponseIter> {
	// Queries all known transactions for the targeted vault account and check if any of
	// them is targeted.
	let wallet = wallet.read().await;
	let transactions_result = wallet.get_all_transactions_iter().await;
	drop(wallet);

	// Check if some of the requests that are open already have a corresponding payment on Stellar
	// and are just waiting to be executed on the parachain
	match transactions_result {
		Err(e) => {
			tracing::error!(
				"Failed to get transactions from Stellar while processing open requests: {e}"
			);
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
pub(crate) async fn retrieve_open_requests_async(
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
