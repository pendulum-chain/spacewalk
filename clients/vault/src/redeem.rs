use std::time::Duration;

use runtime::{InterBtcParachain, RedeemPallet, RequestRedeemEvent, SpacewalkParachain};
use service::{spawn_cancelable, Error as ServiceError, ShutdownSender};
use stellar_relay_lib::sdk::PublicKey;

use crate::{
	execution::*,
	metrics::publish_expected_bitcoin_balance,
	system::{VaultData, VaultIdManager},
	Error,
};

/// Listen for RequestRedeemEvent directed at this vault; upon reception, transfer
/// bitcoin and call execute_redeem
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `payment_margin` - the duration after which a redeem request is considered expired
pub async fn listen_for_redeem_requests(
	shutdown_tx: ShutdownSender,
	parachain_rpc: SpacewalkParachain,
	vault_id_manager: VaultIdManager,
	payment_margin: Duration,
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
				spawn_cancelable(shutdown_tx.subscribe(), async move {
					tracing::info!("Executing redeem #{:?}", event.redeem_id);
					let result = async {
						let request = Request::from_redeem_request(
							event.redeem_id,
							parachain_rpc.get_redeem_request(event.redeem_id).await?,
							payment_margin,
						)?;
						request.pay_and_execute(parachain_rpc, vault).await
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
