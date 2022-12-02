use crate::Error;
use runtime::{
	IssuePallet, IssueRequestsMap, RequestIssueEvent, SpacewalkIssueRequest, SpacewalkParachain,
};
use service::Error as ServiceError;
use std::sync::Arc;
use stellar_relay_lib::sdk::SecretKey;
use tokio::sync::RwLock;

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
	issue_set: Arc<RwLock<IssueRequestsMap>>,
) -> Result<(), ServiceError<Error>> {
	let (sender, mut receiver) = tokio::sync::mpsc::channel(10);

	parachain_rpc
		.on_event::<RequestIssueEvent, _, _, _>(
			|event| async {
				tracing::info!("Received Request Issue event: {:?}", event.issue_id);

				if let Err(e) = sender.send(event).await {
					tracing::error!("failed to send issue id: {:?}", e);
					return
				}
			},
			|error| tracing::error!("Error with RequestIssueEvent: {:?}", error),
		)
		.await?;

	match receiver.recv().await {
		Some(event) => {
			if is_vault(&vault_secret_key, event.vault_stellar_public_key.clone()) {
				// let's get the IssueRequest

				let issue_request = parachain_rpc.get_issue_request(event.issue_id).await?;
				issue_set.write().await.insert(event.issue_id, issue_request);
			}
		},
		None => {
			tracing::error!("Receiver didn't receive any issue id.");
		},
	}

	Ok(())
}

fn is_vault(vault_key: &String, public_key: [u8; 32]) -> bool {
	let vault_keypair: SecretKey = SecretKey::from_encoding(vault_key).unwrap();
	return public_key == *vault_keypair.get_public().as_binary()
}
