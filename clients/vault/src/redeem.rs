use std::{sync::Arc, time::Duration};

use runtime::{RedeemPallet, RequestRedeemEvent, ShutdownSender, SpacewalkParachain};
use service::{spawn_cancelable, Error as ServiceError};

use crate::{execution::*, oracle::OracleAgent, system::VaultIdManager, Error};

/// Listen for RequestRedeemEvent directed at this vault; upon reception, transfer
/// stellar wrapped asset and call execute_redeem
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `payment_margin` - minimum time to the the redeem execution deadline to make the stellar
///   payment.
pub async fn listen_for_redeem_requests(
	shutdown_tx: ShutdownSender,
	parachain_rpc: SpacewalkParachain,
	vault_id_manager: VaultIdManager,
	payment_margin: Duration,
	oracle_agent: Arc<OracleAgent>,
) -> Result<(), ServiceError<Error>> {
	parachain_rpc
		.on_event::<RequestRedeemEvent, _, _, _>(
			|event| async {
				tracing::info!("Received redeem request: {:?}", event);

				let vault = match vault_id_manager.get_vault(&event.vault_id).await {
					Some(x) => x,
					None => return, // event not directed at this vault
				};

				// let _ = publish_expected_bitcoin_balance(&vault, parachain_rpc.clone()).await;

				// within this event callback, we captured the arguments of
				// listen_for_redeem_requests by reference. Since spawn requires static lifetimes,
				// we will need to capture the arguments by value rather than by reference, so clone
				// these:
				let parachain_rpc = parachain_rpc.clone();
				// Spawn a new task so that we handle these events concurrently
				let oracle_agent = oracle_agent.clone();
				spawn_cancelable(shutdown_tx.subscribe(), async move {
					tracing::info!("Executing redeem #{:?}", event.redeem_id);
					let result = async {
						let request = Request::from_redeem_request(
							event.redeem_id,
							parachain_rpc.get_redeem_request(event.redeem_id).await?,
							payment_margin,
						)?;
						request.pay_and_execute(parachain_rpc, vault, oracle_agent).await
					}
					.await;

					match result {
						Ok(_) => tracing::info!(
							"Completed redeem request #{} with amount {}",
							event.redeem_id,
							event.amount
						),
						Err(e) => tracing::error!(
							"Failed to process redeem request #{}: {}",
							event.redeem_id,
							e.to_string()
						),
					}
				});
			},
			|error| tracing::error!("Error reading redeem event: {}", error.to_string()),
		)
		.await?;
	Ok(())
}
