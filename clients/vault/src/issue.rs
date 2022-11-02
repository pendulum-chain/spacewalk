use crate::oracle::{FilterWith, Proof, ScpMessageHandler};
use base64::DecodeError;
use runtime::{
	metadata::runtime_types::spacewalk_primitives::CurrencyId, types::H256, Balance,
	CancelIssueEvent, DefaultIssueRequest, Error, ExecuteIssueEvent, IssueId, IssuePallet,
	RequestIssueEvent, SpacewalkParachain,
};
use service::Error as ServiceError;
use std::{
	collections::HashMap,
	fmt::{Debug, Formatter},
	sync::Arc,
};
use stellar_relay_lib::sdk::{Memo, SecretKey, TransactionEnvelope};
use tokio::sync::mpsc::error::SendError;
use tracing_subscriber::fmt::format;

#[derive(Clone)]
pub enum IssueActions {
	AddFilter(RequestIssueEvent),
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

pub struct IssueChecker {
	issue_id: IssueId,
	amount: Balance,
	asset: CurrencyId,
}

impl IssueChecker {
	fn create(request: &RequestIssueEvent) -> Self {
		IssueChecker {
			issue_id: request.issue_id,
			amount: request.amount,
			asset: request.asset.clone(),
		}
	}
}

impl FilterWith<TransactionEnvelope> for IssueChecker {
	fn name(&self) -> String {
		create_name(self.issue_id)
	}

	fn check_for_processing(&self, param: &TransactionEnvelope) -> bool {
		if let TransactionEnvelope::EnvelopeTypeTx(tx_env) = param {
			if let Memo::MemoHash(hash) = tx_env.tx.memo {
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

pub async fn listen_for_issue_requests(
	parachain_rpc: SpacewalkParachain,
	vault_secret_key: String,
	sender: tokio::sync::mpsc::Sender<IssueActions>,
) -> Result<(), ServiceError> {
	parachain_rpc
		.on_event::<RequestIssueEvent, _, _, _>(
			|event| async {
				tracing::info!("Received Request Issue event: {:?}", event.issue_id);
				if is_vault(&vault_secret_key, event.vault_stellar_public_key.clone()) {
					send_issue_action(&sender, IssueActions::AddFilter(event)).await;
				}
			},
			|error| tracing::error!("Error with RequestIssueEvent: {:?}", error),
		)
		.await?;

	Ok(())
}

pub async fn listen_for_cancel_requests(
	parachain_rpc: SpacewalkParachain,
	sender: tokio::sync::mpsc::Sender<IssueActions>,
) -> Result<(), ServiceError> {
	let sender = &sender;
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

pub async fn listen_for_execute_requests(
	parachain_rpc: SpacewalkParachain,
	sender: tokio::sync::mpsc::Sender<IssueActions>,
) -> Result<(), ServiceError> {
	let sender = &sender;
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

async fn handle_issue_actions(
	handler: &ScpMessageHandler,
	action: IssueActions,
) -> Result<(), ServiceError> {
	match action {
		IssueActions::AddFilter(event) => {
			let checker = IssueChecker::create(&event);
			handler
				.add_filter(Box::new(checker))
				.await
				.map_err(|e| ServiceError::Other(format!("{:?}", e)))
		},
		IssueActions::RemoveFilter(issue_id) => handler
			.remove_filter(create_name(issue_id))
			.await
			.map_err(|e| ServiceError::Other(format!("{:?}", e))),
	}
}

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

pub async fn process_issue_requests(
	parachain_rpc: SpacewalkParachain,
	vault_secret_key: String,
	handler: &ScpMessageHandler,
	mut receiver: tokio::sync::mpsc::Receiver<IssueActions>,
	proof_map: &HashMap<String, Vec<Proof>>,
) -> Result<(), ServiceError> {
	loop {
		tokio::select! {
			Some(msg) = receiver.recv() => {
				handle_issue_actions(handler,msg).await?;
			}

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
	format!("issue_{:?}", base64::encode(issue_id))
}
