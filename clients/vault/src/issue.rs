use std::{convert::TryFrom, sync::Arc, time::Duration};

use futures::{channel::mpsc::Sender, SinkExt};
use sp_runtime::traits::StaticLookup;

use primitives::{stellar::Memo, TransactionEnvelopeExt};
use runtime::{
	CancelIssueEvent, ExecuteIssueEvent, IssueId, IssuePallet, IssueRequestsMap, RequestIssueEvent,
	SpacewalkParachain, StellarPublicKeyRaw, H256,
};
use service::Error as ServiceError;
use stellar_relay_lib::sdk::{PublicKey, TransactionEnvelope, XdrCodec};
use wallet::{
	types::{FilterWith, TransactionFilterParam},
	Ledger, LedgerTxEnvMap,
};

use crate::{
	oracle::{types::Slot, *},
	ArcRwLock, Error, Event,
};

fn is_vault(p1: &PublicKey, p2_raw: [u8; 32]) -> bool {
	return *p1.as_binary() == p2_raw
}

/// Listens for RequestIssueEvent directed at the vault.
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `vault_secret_key` - The secret key of this vault
/// * `event_channel` - the channel over which to signal events
/// * `issues` - a map to save all the new issue requests
pub async fn listen_for_issue_requests(
	parachain_rpc: SpacewalkParachain,
	vault_public_key: PublicKey,
	event_channel: Sender<Event>,
	issues: ArcRwLock<IssueRequestsMap>,
) -> Result<(), ServiceError<Error>> {
	// Use references to prevent 'moved closure' errors
	let parachain_rpc = &parachain_rpc;
	let vault_public_key = &vault_public_key;
	let issues = &issues;
	let event_channel = &event_channel;

	parachain_rpc
		.on_event::<RequestIssueEvent, _, _, _>(
			|event| async move {
				tracing::info!("Received RequestIssueEvent: {:?}", event.issue_id);
				if is_vault(vault_public_key, event.vault_stellar_public_key) {
					// let's get the IssueRequest
					let issue_request_result =
						parachain_rpc.get_issue_request(event.issue_id).await;
					if let Ok(issue_request) = issue_request_result {
						tracing::trace!("Adding issue request to issue map: {:?}", issue_request);
						issues.write().await.insert(event.issue_id, issue_request);

						// try to send the event, but ignore the returned result since
						// the only way it can fail is if the channel is closed
						let _ = event_channel.clone().send(Event::Opened).await;
					} else {
						tracing::error!(
							"Failed to get issue request for issue id: {:?}",
							event.issue_id
						);
					}
				}
			},
			|error| tracing::error!("Error reading RequestIssueEvent: {:?}", error.to_string()),
		)
		.await?;
	Ok(())
}

/// Listens for CancelIssueEvent directed at the vault.
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `issues` - a map to save all the new issue requests
pub async fn listen_for_issue_cancels(
	parachain_rpc: SpacewalkParachain,
	issues: ArcRwLock<IssueRequestsMap>,
) -> Result<(), ServiceError<Error>> {
	let issues = &issues;

	parachain_rpc
		.on_event::<CancelIssueEvent, _, _, _>(
			|event| async move {
				issues.write().await.remove(&event.issue_id);

				tracing::info!("Received CancelIssueEvent: {:?}", event);
			},
			|error| tracing::error!("Error reading CancelIssueEvent: {:?}", error.to_string()),
		)
		.await?;

	Ok(())
}

/// Listens for ExecuteIssueEvent directed at the vault.
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `issues` - a map to save all the new issue requests
pub async fn listen_for_executed_issues(
	parachain_rpc: SpacewalkParachain,
	issues: ArcRwLock<IssueRequestsMap>,
) -> Result<(), ServiceError<Error>> {
	let issues = &issues;

	parachain_rpc
		.on_event::<ExecuteIssueEvent, _, _, _>(
			|event| async move {
				issues.write().await.remove(&event.issue_id);

				tracing::trace!(
					"issue id {:?} was executed and will be removed. issues size: {}",
					event.issue_id,
					issues.read().await.len()
				);
			},
			|error| tracing::error!("Error reading ExecuteIssueEvent: {:?}", error.to_string()),
		)
		.await?;

	Ok(())
}

/// Returns IssueId of the given TransactionEnvelope, or None if it wasn't in a list of
/// IssueRequests
fn get_issue_id_from_tx_env(tx_env: &TransactionEnvelope) -> Option<IssueId> {
	let tx = match tx_env {
		TransactionEnvelope::EnvelopeTypeTx(env) => env.tx.clone(),
		_ => return None,
	};

	match tx.memo {
		Memo::MemoHash(issue_id) => Some(H256::from(issue_id)),
		_ => None,
	}
}

/// Processes all the issue requests
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `oracle_agent` - the agent used to get the proofs
/// * `ledger_env_map` -  a list of TransactionEnvelopes and its corresponding ledger it belongs to
/// * `issues` - a map of all issue requests
pub async fn process_issues_requests(
	parachain_rpc: SpacewalkParachain,
	oracle_agent: Arc<OracleAgent>,
	ledger_env_map: ArcRwLock<LedgerTxEnvMap>,
	issues: ArcRwLock<IssueRequestsMap>,
) -> Result<(), ServiceError<Error>> {
	loop {
		for (ledger, _tx_env) in ledger_env_map.read().await.iter() {
			tokio::spawn(execute_issue(
				parachain_rpc.clone(),
				ledger_env_map.clone(),
				issues.clone(),
				oracle_agent.clone(),
				Slot::from(*ledger),
			));
		}

		tokio::time::sleep(Duration::from_secs(5)).await;
	}
}

/// Execute the issue request for a specific slot
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `ledger_env_map` -  a list of TransactionEnvelopes and its corresponding ledger it belongs to
/// * `issues` - a map of all issue requests
/// * `oracle_agent` - the agent used to get the proofs
/// * `slot` - the slot of the transaction envelope it belongs to
pub async fn execute_issue(
	parachain_rpc: SpacewalkParachain,
	ledger_env_map: ArcRwLock<LedgerTxEnvMap>,
	issues: ArcRwLock<IssueRequestsMap>,
	oracle_agent: Arc<OracleAgent>,
	slot: Slot,
) -> Result<(), ServiceError<Error>> {
	let ledger =
		Ledger::try_from(slot).map_err(|e| ServiceError::VaultError(Error::TryIntoIntError(e)))?;

	// Get the proof of the given slot
	let proof = oracle_agent
		.get_proof(slot)
		.await
		.map_err(|e| ServiceError::OracleError(Error::OracleError(e)))?;

	let (envelopes, tx_set) = proof.encode();

	let mut ledger_env_map = ledger_env_map.write().await;

	// Get the transaction envelope where the ledger belongs to
	if let Some(tx_env) = ledger_env_map.get(&ledger) {
		let tx_env_encoded = {
			let tx_env_xdr = tx_env.to_xdr();
			base64::encode(tx_env_xdr)
		};

		if let Some(issue_id) = get_issue_id_from_tx_env(tx_env) {
			// calls the execute_issue of the `Issue` Pallet
			match parachain_rpc
				.execute_issue(
					issue_id,
					tx_env_encoded.as_bytes(),
					envelopes.as_bytes(),
					tx_set.as_bytes(),
				)
				.await
			{
				Ok(_) => {
					tracing::trace!("Slot {:?} EXECUTED with issue_id: {:?}", ledger, issue_id);
					tracing::info!("Issue request {:?} was executed", issue_id);
					ledger_env_map.remove(&ledger);
					issues.write().await.remove(&issue_id);
				},
				Err(err) if err.is_issue_completed() => {
					tracing::info!("Issue #{} has been completed", issue_id);
				},
				Err(err) => return Err(err.into()),
			}
		}
	}

	Ok(())
}

/// The IssueFilter used for
#[derive(Clone)]
pub struct IssueFilter {
	vault_address: StellarPublicKeyRaw,
}

impl IssueFilter {
	pub fn new(vault_public_key: &PublicKey) -> Result<Self, Error> {
		let vault_address_raw = vault_public_key.clone().into_binary();
		Ok(IssueFilter { vault_address: vault_address_raw })
	}
}

impl FilterWith<TransactionFilterParam<IssueRequestsMap>> for IssueFilter {
	fn is_relevant(&self, param: TransactionFilterParam<IssueRequestsMap>) -> bool {
		let tx = param.0;
		let issue_requests = param.1;

		// try to convert the contained memo_hash (if any) to a h256
		let memo_h256 = match &tx.memo_hash() {
			None => return false,
			Some(memo) => H256::from_slice(memo),
		};

		// check if the memo id is in the list of issues.
		match issue_requests.get(&memo_h256) {
			None => false,
			Some(request) => {
				// Check if the transaction can be used for the issue request by checking if it
				// contains a payment to the vault with an amount greater than 0.
				let tx_env = TransactionEnvelope::from_base64_xdr(tx.envelope_xdr);
				match tx_env {
					Ok(tx_env) => {
						let asset = primitives::AssetConversion::lookup(request.asset);
						if let Ok(asset) = asset {
							let payment_amount_to_vault =
								tx_env.get_payment_amount_for_asset_to(self.vault_address, asset);
							if payment_amount_to_vault > 0 {
								return true
							}
						}
						false
					},
					Err(_) => false,
				}
			},
		}
	}
}
