use crate::oracle::{FilterWith, Proof, ScpMessageHandler};
use runtime::{
	metadata::runtime_types::spacewalk_primitives::CurrencyId, Balance, CancelIssueEvent,
	DefaultIssueRequest, ExecuteIssueEvent, IssueId, IssuePallet, IssueRequests, RequestIssueEvent,
	SpacewalkParachain,
};
use service::Error as ServiceError;
use std::{
	collections::HashMap,
	fmt::{Debug, Formatter},
	sync::Arc,
};
use stellar_relay_lib::sdk::{Memo, SecretKey, TransactionEnvelope};
use tokio::sync::Mutex;

/// Actions to do, depending on the event received.
#[derive(Clone)]
pub enum IssueActions {
	/// Filters the oracle based on the Issue Request
	AddFilter(RequestIssueEvent),
	/// Removes the filter in the oracle, given the issue id
	RemoveFilter(IssueId),
}

impl Debug for IssueActions {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			IssueActions::AddFilter(event) => {
				write!(f, "AddFilter: {:?}", event.issue_id)
			},
			IssueActions::RemoveFilter(id) => {
				write!(f, "RemoveFilter: {:?}", id)
			},
		}
	}
}

/// The struct used to create a filter for oracle.
pub struct IssueChecker {
	issue_id: IssueId,
	request: DefaultIssueRequest,
}

impl IssueChecker {
	fn create(issue_id: IssueId, request: DefaultIssueRequest) -> Self {
		IssueChecker { issue_id, request }
	}
}

impl FilterWith<TransactionEnvelope> for IssueChecker {
	fn name(&self) -> String {
		create_name(self.issue_id)
	}

	fn check_for_processing(&self, param: &TransactionEnvelope) -> bool {
		if let TransactionEnvelope::EnvelopeTypeTx(tx_env) = param {
			if let Memo::MemoHash(hash) = tx_env.tx.memo {
				// todo: check also the amount and the asset
				return self.issue_id.0 == hash
			}
		}
		false
	}
}

async fn send_issue_action(sender: &tokio::sync::mpsc::Sender<IssueActions>, action: IssueActions) {
	let action_name = format!("{:?}", action);
	if let Err(e) = sender.send(action).await {
		tracing::error!("Error while sending {:?}: {:?}", action_name, e.to_string());
	}
}

/// Listens for RequestIssueEvent directed at the vault.
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `vault_secret_key` - The secret key of this vault
/// * `actions_channel` - the channel over which to perform `AddFilter` action for this event
pub async fn listen_for_issue_requests(
	parachain_rpc: SpacewalkParachain,
	vault_secret_key: String,
	actions_channel: tokio::sync::mpsc::Sender<IssueActions>,
) -> Result<(), ServiceError> {
	parachain_rpc
		.on_event::<RequestIssueEvent, _, _, _>(
			|event| async {
				tracing::info!("Received Request Issue event: {:?}", event.issue_id);
				if is_vault(&vault_secret_key, event.vault_stellar_public_key.clone()) {
					send_issue_action(&actions_channel, IssueActions::AddFilter(event)).await;
				}
			},
			|error| tracing::error!("Error with RequestIssueEvent: {:?}", error),
		)
		.await?;

	Ok(())
}

/// Listen for all `CancelIssueEvent`s.
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `actions_channel` - the channel over which to perform `RemoveFilter` action for this event
pub async fn listen_for_cancel_requests(
	parachain_rpc: SpacewalkParachain,
	actions_channel: tokio::sync::mpsc::Sender<IssueActions>,
) -> Result<(), ServiceError> {
	let sender = &actions_channel;
	parachain_rpc
		.on_event::<CancelIssueEvent, _, _, _>(
			|event| async move {
				tracing::info!("Received Cancel Issue event: {:?}", event.issue_id);
				send_issue_action(sender, IssueActions::RemoveFilter(event.issue_id.clone())).await;
			},
			|error| tracing::error!("Error with RequestIssueEvent: {:?}", error),
		)
		.await?;

	Ok(())
}

/// Listen for ExecuteIssueEvent directed at this vault.
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `actions_channel` - the channel over which to perform `RemoveFilter` action for this event
pub async fn listen_for_execute_requests(
	parachain_rpc: SpacewalkParachain,
	actions_channel: tokio::sync::mpsc::Sender<IssueActions>,
) -> Result<(), ServiceError> {
	let sender = &actions_channel;
	parachain_rpc
		.on_event::<ExecuteIssueEvent, _, _, _>(
			|event| async move {
				tracing::info!("Received Execute Issue event: {:?}", event.issue_id);
				send_issue_action(sender, IssueActions::RemoveFilter(event.issue_id.clone())).await;
			},
			|error| tracing::error!("Error with RequestIssueEvent: {:?}", error),
		)
		.await?;

	Ok(())
}

/// adds/removes issue requests in the set, and adds/removes filters in the oracle,
/// depending on the Issue Action.
///
/// # Arguments
///
/// * `handler` - the oracle handler
/// * `action` - the `IssueAction`
/// * `issue_set` - the list of current issue requests
async fn handle_issue_actions(
	parachain_rpc: SpacewalkParachain,
	handler: &ScpMessageHandler,
	action: IssueActions,
	issue_set: Arc<Mutex<IssueRequests>>,
) -> Result<(), ServiceError> {
	let mut issue_set = issue_set.lock().await;

	match action {
		IssueActions::AddFilter(event) => {
			let request = parachain_rpc.get_issue_request(event.issue_id).await?;
			// create the filter
			let checker = IssueChecker::create(event.issue_id, request.clone());

			// send the filter to the oracle
			handler
				.add_filter(Box::new(checker))
				.await
				.map_err(|e| ServiceError::Other(format!("{:?}", e)))?;

			tracing::debug!("added: {:?}", event);
			// save this request
			issue_set.insert(event.issue_id, request);
		},
		IssueActions::RemoveFilter(issue_id) => {
			// signal the oracle to remove this filter
			handler
				.remove_filter(create_name(issue_id))
				.await
				.map_err(|e| ServiceError::Other(format!("{:?}", e)))?;

			// remove this request
			issue_set.remove(&issue_id);
		},
	}
	Ok(())
}

/// executes issue requests
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `proofs` - a list of proofs to execute
pub async fn execute_issue(
	parachain_rpc: SpacewalkParachain,
	proofs: Vec<Proof>,
) -> Result<(), ServiceError> {
	for proof in proofs {
		let (tx_env_encoded, envelopes_encoded, tx_set_encoded) = proof.encode();

		if let Some(issue_id) = get_issue_id_of_proof(&proof) {
			match parachain_rpc
				.execute_issue(
					issue_id,
					tx_env_encoded.as_bytes(),
					envelopes_encoded.as_bytes(),
					tx_set_encoded.as_bytes(),
				)
				.await
			{
				Ok(_) => (),
				Err(err) if err.is_issue_completed() => {
					tracing::info!("Issue #{} has been completed", issue_id);
				},
				Err(err) => return Err(err.into()),
			}
		}
	}

	Ok(())
}

/// processes events that are directed to this vault.
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `handler` - the oracle handler
/// * `proofs` - a list of proofs to execute
/// * `action_receiver` - a channel that accepts IssueActions
pub async fn process_issue_events(
	parachain_rpc: SpacewalkParachain,
	handler: Arc<Mutex<ScpMessageHandler>>,
	mut action_receiver: tokio::sync::mpsc::Receiver<IssueActions>,
	issue_set: Arc<Mutex<IssueRequests>>,
) -> Result<(), ServiceError> {
	let handler = handler.lock().await;

	loop {
		tokio::select! {
			Some(msg) = action_receiver.recv() => {
				handle_issue_actions(parachain_rpc.clone(),&handler,msg,issue_set.clone()).await?;
			}
			// get all proofs
			Ok(proofs) = handler.get_pending_proofs() => {
				tokio::spawn(execute_issue(parachain_rpc.clone(),proofs));
			}
		}
	}
}

fn is_vault(vault_key: &String, public_key: [u8; 32]) -> bool {
	let vault_keypair: SecretKey = SecretKey::from_encoding(vault_key).unwrap();
	return public_key == *vault_keypair.get_public().as_binary()
}

fn get_issue_id_from_memo(memo: &Memo) -> Option<IssueId> {
	match memo {
		Memo::MemoHash(hash) => Some(IssueId::from_slice(hash)),
		_ => None,
	}
}

fn get_issue_id_of_proof(proof: &Proof) -> Option<IssueId> {
	let memo = proof.get_memo()?;
	get_issue_id_from_memo(memo)
}

fn create_name(issue_id: IssueId) -> String {
	format!("issue_{:?}", hex::encode(issue_id))
}
