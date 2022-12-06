use crate::Error;
use runtime::{
	CancelIssueEvent, ExecuteIssueEvent, IssuePallet, IssueRequestsMap, RequestIssueEvent,
	SpacewalkIssueRequest, SpacewalkParachain, StaticEvent, H256,
};
use service::Error as ServiceError;
use std::{fmt::Debug, string::FromUtf8Error, sync::Arc};
use stellar_relay_lib::sdk::SecretKey;
use tokio::sync::RwLock;
use wallet::{
	types::{FilterWith, TransactionFilterParam},
	StellarWallet,
};

fn is_vault(secret_key: &SecretKey, public_key: [u8; 32]) -> bool {
	return public_key == *secret_key.get_public().as_binary()
}

async fn listen_event<T>(
	parachain_rpc: &SpacewalkParachain,
) -> Result<Option<T>, ServiceError<Error>>
where
	T: StaticEvent + Debug,
{
	let (sender, mut receiver) = tokio::sync::mpsc::channel(10);

	parachain_rpc
		.on_event::<T, _, _, _>(
			|event| async {
				tracing::info!("Received Event: {:?}", event);

				if let Err(e) = sender.send(event).await {
					tracing::error!("failed to send issue id: {:?}", e);
					return
				}
			},
			|error| tracing::error!("Error: {:?}", error),
		)
		.await?;

	Ok(receiver.recv().await)
}

/// Listens for RequestIssueEvent directed at the vault.
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `vault_secret_key` - The secret key of this vault
/// * `issues` - a map to save all the new issue requests
pub async fn listen_for_issue_requests(
	parachain_rpc: SpacewalkParachain,
	vault_secret_key: SecretKey,
	issues: Arc<RwLock<IssueRequestsMap>>,
) -> Result<(), ServiceError<Error>> {
	match listen_event::<RequestIssueEvent>(&parachain_rpc).await? {
		Some(event) => {
			let event: RequestIssueEvent = event;
			if is_vault(&vault_secret_key, event.vault_stellar_public_key.clone()) {
				// let's get the IssueRequest
				let issue_request = parachain_rpc.get_issue_request(event.issue_id).await?;
				issues.write().await.insert(event.issue_id, issue_request);
			}
		},
		None => {
			tracing::trace!("Receiver didn't receive any Issue Request.");
		},
	}

	Ok(())
}

pub async fn listen_for_issue_cancels(
	parachain_rpc: SpacewalkParachain,
	issues: Arc<RwLock<IssueRequestsMap>>,
) -> Result<(), ServiceError<Error>> {
	match listen_event::<CancelIssueEvent>(&parachain_rpc).await? {
		Some(event) => {
			issues.write().await.remove(&event.issue_id);
		},
		None => {
			tracing::trace!("Receiver didn't receive any cancel issue event.");
		},
	}
	Ok(())
}

pub async fn listen_for_executed_issues(
	parachain_rpc: SpacewalkParachain,
	issues: Arc<RwLock<IssueRequestsMap>>,
) -> Result<(), ServiceError<Error>> {
	match listen_event::<ExecuteIssueEvent>(&parachain_rpc).await? {
		Some(event) => {
			issues.write().await.remove(&event.issue_id);
		},
		None => {
			tracing::trace!("Receiver didn't receive any execute issue event.");
		},
	}

	Ok(())
}

pub async fn process_issue_requests(
	parachain_rpc: SpacewalkParachain,
	wallet: Arc<StellarWallet>,
	issues: Arc<RwLock<IssueRequestsMap>>,
) -> Result<(), ServiceError<Error>> {
	// TODO: implement this
	Ok(())
}

#[derive(Clone)]
pub struct IssueFilter;

impl FilterWith<TransactionFilterParam<IssueRequestsMap>> for IssueFilter {
	fn is_relevant(&self, param: TransactionFilterParam<IssueRequestsMap>) -> bool {
		match String::from_utf8(param.0.memo_type.clone()) {
			Ok(memo_type) if memo_type == "hash" =>
				if let Some(memo) = &param.0.memo {
					let memo = H256::from_slice(memo);

					return param.1.contains_key(&memo)
				},
			Err(e) => {
				tracing::warn!("Failed to retrieve memo type: {:?}", e);
			},
			_ => {},
		}

		false
	}
}
