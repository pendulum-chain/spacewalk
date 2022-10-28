use crate::oracle::{FilterWith, ScpMessageHandler};
use runtime::{
	types::H256, Balance, CancelIssueEvent, DefaultIssueRequest, ExecuteIssueEvent,
	RequestIssueEvent, SpacewalkParachain,
};
use service::Error as ServiceError;
use std::{collections::HashMap, sync::Arc};
use stellar_relay::sdk::{Memo, SecretKey, TransactionEnvelope};
use tokio::sync::mpsc::error::SendError;

type IssueId = H256;

fn create_name(issue_id: IssueId) -> String {
	format!("issue_{:?}", base64::encode(issue_id))
}

pub enum IssueActions {
	AddFilter(RequestIssueEvent),
	RemoveFilter(IssueId),
}

pub struct IssueChecker {
	issue_id: IssueId,
	amount: Balance,
	asset: runtime::metadata::runtime_types::spacewalk_primitives::CurrencyId,
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
					if let Err(e) = sender.send(IssueActions::AddFilter(event)).await {
						tracing::error!(
							"Error while sending AddFilter message: {:?}",
							e.to_string()
						);
					}
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

				if let Err(e) =
					sender.send(IssueActions::RemoveFilter(event.issue_id.clone())).await
				{
					tracing::error!(
						"Error while sending RemoveFilter message: {:?}",
						e.to_string()
					);
				}
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

				if let Err(e) =
					sender.send(IssueActions::RemoveFilter(event.issue_id.clone())).await
				{
					tracing::error!(
						"Error while sending RemoveFilter message: {:?}",
						e.to_string()
					);
				}
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

pub async fn process_issue_requests(
	parachain_rpc: SpacewalkParachain,
	vault_secret_key: String,
	handler: &ScpMessageHandler,
	mut receiver: tokio::sync::mpsc::Receiver<IssueActions>,
) -> Result<(), ServiceError> {
	loop {
		tokio::select! {
			Some(msg) = receiver.recv() => {
				handle_issue_actions(handler,msg).await?;
			}

			Ok(proofs) = handler.get_pending_proofs() => {
				if proofs.len() > 0 {
					tokio::spawn(async move {
						for proof in proofs {
							//todo:: execute_issue
						}
					});
				}
			}

		}
	}
}

fn is_vault(vault_key: &String, public_key: [u8; 32]) -> bool {
	let vault_keypair: SecretKey = SecretKey::from_encoding(vault_key).unwrap();
	return public_key == *vault_keypair.get_public().as_binary()
}
