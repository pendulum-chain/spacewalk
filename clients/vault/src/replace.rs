use std::{sync::Arc, time::Duration};

use futures::{channel::mpsc::Sender, future::try_join3, SinkExt};
use tokio::sync::RwLock;

use runtime::{
	AcceptReplaceEvent, CollateralBalancesPallet, ExecuteReplaceEvent, PrettyPrint, ReplacePallet,
	RequestReplaceEvent, ShutdownSender, SpacewalkParachain, UtilFuncs, VaultId,
	VaultRegistryPallet,
};
use service::{spawn_cancelable, Error as ServiceError};
use wallet::StellarWallet;

use crate::{
	cancellation::Event, error::Error, execution::Request, oracle::ProofExt, system::VaultIdManager,
};

/// Listen for AcceptReplaceEvent directed at this vault and continue the replacement
/// procedure by transferring the corresponding stellar assets and calling execute_replace
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
pub async fn listen_for_accept_replace(
	shutdown_tx: ShutdownSender,
	parachain_rpc: SpacewalkParachain,
	vault_id_manager: VaultIdManager,
	payment_margin: Duration,
	proof_ops: Arc<RwLock<dyn ProofExt>>,
) -> Result<(), ServiceError<Error>> {
	let parachain_rpc = &parachain_rpc;
	let vault_id_manager = &vault_id_manager;
	let shutdown_tx = &shutdown_tx;
	let proof_ops = &proof_ops;
	parachain_rpc
		.on_event::<AcceptReplaceEvent, _, _, _>(
			|event| async move {
				let vault = match vault_id_manager.get_vault(&event.old_vault_id).await {
					Some(x) => x,
					None => return, // event not directed at this vault
				};
				tracing::info!("Received accept replace event: {:?}", event);

				// let _ = publish_expected_bitcoin_balance(&vault, parachain_rpc.clone()).await;

				// within this event callback, we captured the arguments of
				// listen_for_redeem_requests by reference. Since spawn requires static lifetimes,
				// we will need to capture the arguments by value rather than by reference, so clone
				// these:
				let parachain_rpc = parachain_rpc.clone();
				let proof_ops = proof_ops.clone();
				// Spawn a new task so that we handle these events concurrently
				spawn_cancelable(shutdown_tx.subscribe(), async move {
					tracing::info!("Executing accept replace #{:?}", event.replace_id);

					let result = async {
						let request = Request::from_replace_request(
							event.replace_id,
							parachain_rpc.get_replace_request(event.replace_id).await?,
							payment_margin,
						)?;
						request.pay_and_execute(parachain_rpc, vault, proof_ops).await
					}
					.await;

					match result {
						Ok(_) => tracing::info!(
							"Completed accept replace request #{} with amount {}",
							event.replace_id,
							event.amount
						),
						Err(e) => tracing::error!(
							"Failed to process accept replace request #{}: {}",
							event.replace_id,
							e.to_string()
						),
					}
				});
			},
			|error| tracing::error!("Error reading accept_replace_event: {}", error.to_string()),
		)
		.await?;
	Ok(())
}

/// Listen for RequestReplaceEvent, and attempt to accept it
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `event_channel` - the channel over which to signal events
/// * `accept_replace_requests` - if true, we attempt to accept replace requests
pub async fn listen_for_replace_requests(
	parachain_rpc: SpacewalkParachain,
	vault_id_manager: VaultIdManager,
	event_channel: Sender<Event>,
	accept_replace_requests: bool,
) -> Result<(), ServiceError<Error>> {
	let parachain_rpc = &parachain_rpc;
	let vault_id_manager = &vault_id_manager;
	let event_channel = &event_channel;
	parachain_rpc
		.on_event::<RequestReplaceEvent, _, _, _>(
			|event| async move {
				if parachain_rpc.is_this_vault(&event.old_vault_id) {
					// don't respond to requests we placed ourselves
					return
				}

				tracing::info!(
					"Received replace request from {} for amount {}",
					event.old_vault_id.pretty_print(),
					event.amount
				);

				if accept_replace_requests {
					for (vault_id, wallet) in vault_id_manager.get_vault_stellar_wallets().await {
						match handle_replace_request(
							parachain_rpc.clone(),
							wallet.clone(),
							&event,
							&vault_id,
						)
						.await
						{
							Ok(_) => {
								tracing::info!(
									"[{}] Accepted replace request from {}",
									vault_id.pretty_print(),
									event.old_vault_id.pretty_print()
								);
								// try to send the event, but ignore the returned result since
								// the only way it can fail is if the channel is closed
								let _ = event_channel.clone().send(Event::Opened).await;

								return // no need to iterate over the rest of the vault ids
							},
							Err(e) => tracing::error!(
								"[{}] Failed to accept replace request from {}: {}",
								vault_id.pretty_print(),
								event.old_vault_id.pretty_print(),
								e.to_string()
							),
						}
					}
				}
			},
			|error| tracing::error!("Error reading replace event: {}", error.to_string()),
		)
		.await?;
	Ok(())
}

/// Attempts to accept a replace request. Does not retry RPC calls upon
/// failure, since nothing is at stake at this point
pub async fn handle_replace_request<
	'a,
	P: CollateralBalancesPallet + ReplacePallet + VaultRegistryPallet,
>(
	parachain_rpc: P,
	wallet: Arc<RwLock<StellarWallet>>,
	event: &'a RequestReplaceEvent,
	vault_id: &'a VaultId,
) -> Result<(), Error> {
	let collateral_currency = vault_id.collateral_currency();

	let (required_replace_collateral, current_collateral, used_collateral) = try_join3(
		parachain_rpc.get_required_collateral_for_wrapped(event.amount, collateral_currency),
		parachain_rpc.get_vault_total_collateral(vault_id.clone()),
		parachain_rpc.get_required_collateral_for_vault(vault_id.clone()),
	)
	.await?;

	let total_required_collateral = required_replace_collateral.saturating_add(used_collateral);

	if current_collateral < total_required_collateral {
		Err(Error::InsufficientFunds)
	} else {
		let wallet = wallet.read().await;
		Ok(parachain_rpc
			.accept_replace(
				vault_id,
				&event.old_vault_id,
				event.amount,
				0, // do not lock any additional collateral
				wallet.get_public_key_raw(),
			)
			.await?)
	}
}

/// Listen for ExecuteReplaceEvent directed at this vault and continue the replacement
/// procedure by transferring bitcoin and calling execute_replace
///
/// # Arguments
///
/// * `event_channel` - the channel over which to signal events
pub async fn listen_for_execute_replace(
	parachain_rpc: SpacewalkParachain,
	event_channel: Sender<Event>,
) -> Result<(), ServiceError<Error>> {
	let event_channel = &event_channel;
	let parachain_rpc = &parachain_rpc;
	parachain_rpc
		.on_event::<ExecuteReplaceEvent, _, _, _>(
			|event| async move {
				if &event.new_vault_id.account_id == parachain_rpc.get_account_id() {
					tracing::info!("Received event: execute replace #{:?}", event.replace_id);
					// try to send the event, but ignore the returned result since
					// the only way it can fail is if the channel is closed
					let _ = event_channel.clone().send(Event::Executed(event.replace_id)).await;
				}
			},
			|error| tracing::error!("Error reading redeem event: {}", error.to_string()),
		)
		.await?;
	Ok(())
}
