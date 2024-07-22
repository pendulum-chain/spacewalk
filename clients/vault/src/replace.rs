use std::{sync::Arc, time::Duration};

use futures::{channel::mpsc::Sender, future::try_join3, SinkExt};
use tokio::sync::RwLock;

use crate::{
	cancellation::Event, error::Error, oracle::OracleAgent, requests::Request,
	system::VaultIdManager,
};
use runtime::{
	AcceptReplaceEvent, CollateralBalancesPallet, ExecuteReplaceEvent, PrettyPrint, ReplacePallet,
	RequestReplaceEvent, ShutdownSender, SpacewalkParachain, UtilFuncs, VaultId,
	VaultRegistryPallet,
};
use service::{spawn_cancelable, Error as ServiceError};
use wallet::StellarWallet;

/// Listen for AcceptReplaceEvent directed at this vault and continue the replacement
/// procedure by transferring the corresponding Stellar assets and calling execute_replace.
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
pub async fn listen_for_accept_replace(
	shutdown_tx: ShutdownSender,
	parachain_rpc: SpacewalkParachain,
	vault_id_manager: VaultIdManager,
	payment_margin: Duration,
	oracle_agent: Arc<OracleAgent>,
) -> Result<(), ServiceError<Error>> {
	let parachain_rpc = &parachain_rpc;
	let vault_id_manager = &vault_id_manager;
	let shutdown_tx = &shutdown_tx;
	let oracle_agent = &oracle_agent;
	parachain_rpc
		.on_event::<AcceptReplaceEvent, _, _, _>(
			|event| async move {
				let vault = match vault_id_manager.get_vault(&event.old_vault_id).await {
					Some(x) => x,
					None => return, // event not directed at this vault
				};
				// within this event callback, we captured the arguments of
				// listen_for_redeem_requests by reference. Since spawn requires static lifetimes,
				// we will need to capture the arguments by value rather than by reference, so clone
				// these:
				let parachain_rpc = parachain_rpc.clone();
				let oracle_agent = oracle_agent.clone();
				// Spawn a new task so that we handle these events concurrently
				spawn_cancelable(shutdown_tx.subscribe(), async move {
					tracing::info!(
						"Received AcceptReplaceEvent {:?} for vault {:?}. Trying to execute...",
						event,
						vault
					);

					let result = async {
						let request = Request::from_replace_request(
							event.replace_id,
							parachain_rpc.get_replace_request(event.replace_id).await?,
							payment_margin,
						)?;
						request.pay_and_execute(parachain_rpc, vault, oracle_agent).await
					}
					.await;

					match result {
						Ok(_) => tracing::info!(
							"Completed Replace request #{:?} with amount {}",
							event.replace_id,
							event.amount
						),
						Err(e) => tracing::error!(
							"Failed to process Replace request #{:?} due to error: {}",
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
					"Received RequestReplaceEvent from {} for amount {}",
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
									"Accepted Replace from {} with [{}]",
									event.old_vault_id.pretty_print(),
									vault_id.pretty_print(),
								);
								// try to send the event, but ignore the returned result since
								// the only way it can fail is if the channel is closed
								let _ = event_channel.clone().send(Event::Opened).await;

								return // no need to iterate over the rest of the vault ids
							},
							Err(e) => tracing::error!(
								"Failed to accept Replace from {} with [{}] due to error: {}",
								event.old_vault_id.pretty_print(),
								vault_id.pretty_print(),
								e.to_string()
							),
						}
					}
				}
			},
			|error| tracing::error!("Request Replace: Failed reading replace event: {error:?}"),
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
	let wrapped_currency = vault_id.wrapped_currency();

	let (required_replace_collateral, current_collateral, used_collateral) = try_join3(
		parachain_rpc.get_required_collateral_for_wrapped(
			event.amount,
			wrapped_currency,
			collateral_currency,
		),
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
				wallet.public_key_raw(),
			)
			.await?)
	}
}

/// Listen for ExecuteReplaceEvent directed at this vault and continue the replacement
/// procedure by transferring the corresponding Stellar asset and calling execute_replace.
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
					tracing::info!("Received ExecuteReplaceEvent for this vault: {:?}", event);
					// try to send the event, but ignore the returned result since
					// the only way it can fail is if the channel is closed
					let _ = event_channel.clone().send(Event::Executed(event.replace_id)).await;
				}
			},
			|error| tracing::error!("Request Replace : Error reading event: {}", error.to_string()),
		)
		.await?;
	Ok(())
}

#[cfg(all(test, feature = "standalone-metadata"))]
mod tests {
	use std::{path::Path, sync::Arc};

	use async_trait::async_trait;

	use crate::ArcRwLock;
	use runtime::{
		AccountId, Balance, CurrencyId, Error as RuntimeError, SpacewalkReplaceRequest,
		SpacewalkVault, StellarPublicKeyRaw, VaultId, H256,
	};

	use wallet::{keys::get_source_secret_key_from_env, StellarWallet};

	use super::*;

	macro_rules! assert_err {
		($result:expr, $err:pat) => {{
			match $result {
				Err($err) => (),
				Ok(v) => panic!("assertion failed: Ok({:?})", v),
				_ => panic!("expected: Err($err)"),
			}
		}};
	}

	mockall::mock! {
		Provider {}

		#[async_trait]
		pub trait VaultRegistryPallet {
			async fn get_vault(&self, vault_id: &VaultId) -> Result<SpacewalkVault, RuntimeError>;
			async fn get_vaults_by_account_id(&self, account_id: &AccountId) -> Result<Vec<VaultId>, RuntimeError>;
			async fn get_all_vaults(&self) -> Result<Vec<SpacewalkVault>, RuntimeError>;
			async fn register_vault(&self, vault_id: &VaultId, collateral: u128) -> Result<(), RuntimeError>;
			async fn deposit_collateral(&self, vault_id: &VaultId, amount: u128) -> Result<(), RuntimeError>;
			async fn withdraw_collateral(&self, vault_id: &VaultId, amount: u128) -> Result<(), RuntimeError>;
			async fn get_public_key(&self) -> Result<Option<StellarPublicKeyRaw>, RuntimeError>;
			async fn register_public_key(&self, public_key: StellarPublicKeyRaw) -> Result<(), RuntimeError>;
			async fn get_required_collateral_for_wrapped(&self, amount: u128, wrapped_currency: CurrencyId, collateral_currency: CurrencyId) -> Result<u128, RuntimeError>;
			async fn get_required_collateral_for_vault(&self, vault_id: VaultId) -> Result<u128, RuntimeError>;
			async fn get_vault_total_collateral(&self, vault_id: VaultId) -> Result<u128, RuntimeError>;
			async fn get_collateralization_from_vault(&self, vault_id: VaultId, only_issued: bool) -> Result<u128, RuntimeError>;
		}



		#[async_trait]
		pub trait ReplacePallet {
			async fn request_replace(&self, vault_id: &VaultId, amount: u128) -> Result<(), RuntimeError>;
			async fn withdraw_replace(&self, vault_id: &VaultId, amount: u128) -> Result<(), RuntimeError>;
			async fn accept_replace(&self, new_vault: &VaultId, old_vault: &VaultId, amount: u128, collateral: u128, stellar_address: StellarPublicKeyRaw) -> Result<(), RuntimeError>;
			async fn execute_replace(&self, replace_id: H256, tx_env: &[u8], scp_envs: &[u8], tx_set: &[u8]) -> Result<(), RuntimeError>;
			async fn cancel_replace(&self, replace_id: H256) -> Result<(), RuntimeError>;
			async fn get_new_vault_replace_requests(&self, account_id: AccountId) -> Result<Vec<(H256, SpacewalkReplaceRequest)>, RuntimeError>;
			async fn get_old_vault_replace_requests<T: 'static>(
				&self,
				account_id: AccountId,
				filter: Box<dyn Fn((H256, SpacewalkReplaceRequest)) -> Option<T> + Send>,
			) -> Result<Vec<T>, RuntimeError>;
			async fn get_replace_period(&self) -> Result<u32, RuntimeError>;
			async fn get_replace_request(&self, replace_id: H256) -> Result<SpacewalkReplaceRequest, RuntimeError>;
			async fn get_replace_dust_amount(&self) -> Result<u128, RuntimeError>;
		}


		#[async_trait]
		pub trait CollateralBalancesPallet {
			async fn get_free_balance(&self, currency_id: CurrencyId) -> Result<Balance, RuntimeError>;
			async fn get_native_balance_for_id(&self, id: &AccountId) -> Result<Balance, RuntimeError>;
			async fn get_free_balance_for_id(&self, id: AccountId, currency_id: CurrencyId) -> Result<Balance, RuntimeError>;
			async fn get_reserved_balance(&self, currency_id: CurrencyId) -> Result<Balance, RuntimeError>;
			async fn get_reserved_balance_for_id(&self, id: AccountId, currency_id: CurrencyId) -> Result<Balance, RuntimeError>;
			async fn transfer_to(&self, recipient: &AccountId, amount: u128, currency_id: CurrencyId) -> Result<(), RuntimeError>;
		}
	}

	impl Clone for MockProvider {
		fn clone(&self) -> Self {
			// NOTE: expectations dropped
			Self::default()
		}
	}

	fn dummy_vault_id() -> VaultId {
		VaultId::new(
			subxt::ext::sp_runtime::AccountId32::new([1u8; 32]).into(),
			CurrencyId::XCM(0),
			CurrencyId::Native,
		)
	}

	fn wallet(is_public_network: bool, path: &Path) -> ArcRwLock<StellarWallet> {
		let wallet = StellarWallet::from_secret_encoded_with_cache(
			&get_source_secret_key_from_env(is_public_network),
			is_public_network,
			path.to_str().expect("should return a string").to_string(),
		)
		.unwrap();
		Arc::new(RwLock::new(wallet))
	}

	#[tokio::test]
	async fn test_handle_replace_request_with_insufficient_balance() {
		let is_public_network = false;
		let tmp = tempdir::TempDir::new("spacewalk-parachain-").expect("failed to create tempdir");
		let wallet_arc = wallet(is_public_network, tmp.path());

		let mut parachain_rpc = MockProvider::default();
		parachain_rpc
			.expect_get_required_collateral_for_wrapped()
			.returning(|_, _, _| Ok(51));
		parachain_rpc.expect_get_required_collateral_for_vault().returning(|_| Ok(50));
		parachain_rpc.expect_get_vault_total_collateral().returning(|_| Ok(100));

		let event = RequestReplaceEvent {
			old_vault_id: dummy_vault_id(),
			amount: Default::default(),
			asset: subxt::utils::Static(Default::default()),
			griefing_collateral: Default::default(),
		};
		assert_err!(
			handle_replace_request(parachain_rpc, wallet_arc, &event, &dummy_vault_id()).await,
			Error::InsufficientFunds
		);
	}

	#[tokio::test]
	async fn test_handle_replace_request_with_sufficient_balance() {
		let is_public_network = false;
		let tmp = tempdir::TempDir::new("spacewalk-parachain-").expect("failed to create tempdir");
		let wallet_arc = wallet(is_public_network, tmp.path());

		let mut parachain_rpc = MockProvider::default();
		parachain_rpc
			.expect_get_required_collateral_for_wrapped()
			.returning(|_, _, _| Ok(50));
		parachain_rpc.expect_get_required_collateral_for_vault().returning(|_| Ok(50));
		parachain_rpc.expect_get_vault_total_collateral().returning(|_| Ok(100));
		parachain_rpc.expect_accept_replace().returning(|_, _, _, _, _| Ok(()));

		let event = RequestReplaceEvent {
			old_vault_id: dummy_vault_id(),
			amount: Default::default(),
			asset: subxt::utils::Static(Default::default()),
			griefing_collateral: Default::default(),
		};
		handle_replace_request(parachain_rpc, wallet_arc, &event, &dummy_vault_id())
			.await
			.unwrap();
	}
}
