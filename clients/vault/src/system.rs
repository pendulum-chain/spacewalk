use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use async_trait::async_trait;
use clap::Parser;
use futures::{
	future::{join, join_all},
	TryFutureExt,
};
use git_version::git_version;
use tokio::{sync::RwLock, time::sleep};

use runtime::{
	CurrencyId, Error as RuntimeError, PrettyPrint, RegisterVaultEvent, SpacewalkParachain,
	UtilFuncs, VaultId,
};
use service::{wait_or_shutdown, Error as ServiceError, Service, ShutdownSender};

use crate::{error::Error, stellar_wallet::StellarWallet, CHAIN_HEIGHT_POLLING_INTERVAL};

pub const VERSION: &str = git_version!(args = ["--tags"], fallback = "unknown");
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Clone)]
pub struct VaultData {
	pub vault_id: VaultId,
	pub stellar_wallet: StellarWallet,
}

#[derive(Clone)]
pub struct VaultIdManager {
	vault_data: Arc<RwLock<HashMap<VaultId, VaultData>>>,
	spacewalk_parachain: SpacewalkParachain,
	stellar_wallet: StellarWallet,
}

impl VaultIdManager {
	pub fn new(spacewalk_parachain: SpacewalkParachain, stellar_wallet: StellarWallet) -> Self {
		Self {
			vault_data: Arc::new(RwLock::new(HashMap::new())),
			spacewalk_parachain,
			stellar_wallet,
		}
	}

	async fn add_vault_id(&self, vault_id: VaultId) -> Result<(), Error> {
		// TODO what is this about?
		// tracing::info!("Adding keys from past issues...");
		// issue::add_keys_from_past_issue_request(&btc_rpc, &self.spacewalk_parachain, &vault_id)
		// 	.await?;

		let data =
			VaultData { vault_id: vault_id.clone(), stellar_wallet: self.stellar_wallet.clone() };

		self.vault_data.write().await.insert(vault_id, data.clone());

		Ok(())
	}

	pub async fn fetch_vault_ids(&self) -> Result<(), Error> {
		for vault_id in self
			.spacewalk_parachain
			.get_vaults_by_account_id(self.spacewalk_parachain.get_account_id())
			.await?
		{
			match is_vault_registered(&self.spacewalk_parachain, &vault_id).await {
				Err(Error::RuntimeError(RuntimeError::VaultLiquidated)) => {
					tracing::error!(
						"[{}] Vault is liquidated -- not going to process events for this vault.",
						vault_id.pretty_print()
					);
				},
				Ok(_) => {
					self.add_vault_id(vault_id.clone()).await?;
				},
				Err(x) => return Err(x),
			}
		}
		Ok(())
	}

	pub async fn listen_for_vault_id_registrations(self) -> Result<(), ServiceError<Error>> {
		Ok(self
			.spacewalk_parachain
			.on_event::<RegisterVaultEvent, _, _, _>(
				|event| async {
					let vault_id = event.vault_id;
					if self.spacewalk_parachain.is_this_vault(&vault_id) {
						tracing::info!("New vault registered: {}", vault_id.pretty_print());
						let _ = self.add_vault_id(vault_id).await;
					}
				},
				|err| tracing::error!("Error (RegisterVaultEvent): {}", err.to_string()),
			)
			.await?)
	}

	pub async fn get_stellar_wallet(&self, vault_id: &VaultId) -> Option<StellarWallet> {
		self.vault_data.read().await.get(vault_id).map(|x| x.stellar_wallet.clone())
	}

	pub async fn get_vault(&self, vault_id: &VaultId) -> Option<VaultData> {
		self.vault_data.read().await.get(vault_id).cloned()
	}

	pub async fn get_entries(&self) -> Vec<VaultData> {
		self.vault_data.read().await.iter().map(|(_, value)| value.clone()).collect()
	}

	pub async fn get_vault_ids(&self) -> Vec<VaultId> {
		self.vault_data
			.read()
			.await
			.iter()
			.map(|(vault_id, _)| vault_id.clone())
			.collect()
	}

	// TODO think about this. It is probably not needed because we don't use deposit addresses and
	// use one stellar wallet for everything
	// But we could in theory let a vault have multiple stellar wallets
	pub async fn get_vault_stellar_wallets(&self) -> Vec<(VaultId, StellarWallet)> {
		self.vault_data
			.read()
			.await
			.iter()
			.map(|(vault_id, data)| (vault_id.clone(), data.stellar_wallet.clone()))
			.collect()
	}
}

fn parse_collateral_and_amount(
	s: &str,
) -> Result<(String, Option<u128>), Box<dyn std::error::Error + Send + Sync + 'static>> {
	let pos = s
		.find('=')
		.ok_or_else(|| format!("invalid CurrencyId=amount: no `=` found in `{}`", s))?;

	let val = &s[pos + 1..];
	Ok((s[..pos].to_string(), if val.contains("faucet") { None } else { Some(val.parse()?) }))
}

#[derive(Parser, Clone, Debug)]
pub struct VaultServiceConfig {
	#[clap(long, help = "The Stellar secret key that is used to sign transactions.")]
	pub stellar_vault_secret_key: String,

	/// Pass the faucet URL for auto-registration.
	#[clap(long)]
	pub faucet_url: Option<String>,

	/// Automatically register the vault with the given amount of collateral
	#[clap(long, value_parser = parse_collateral_and_amount)]
	pub auto_register: Vec<(String, Option<u128>)>,
}

pub struct VaultService {
	spacewalk_parachain: SpacewalkParachain,
	config: VaultServiceConfig,
	shutdown: ShutdownSender,
}

#[async_trait]
impl Service<VaultServiceConfig, Error> for VaultService {
	const NAME: &'static str = NAME;
	const VERSION: &'static str = VERSION;

	fn new_service(
		spacewalk_parachain: SpacewalkParachain,
		config: VaultServiceConfig,
		shutdown: ShutdownSender,
	) -> Self {
		VaultService::new(spacewalk_parachain, config, shutdown)
	}

	async fn start(&self) -> Result<(), ServiceError<Error>> {
		self.run_service().await
	}
}

async fn run_and_monitor_tasks(
	shutdown_tx: ShutdownSender,
	items: Vec<(&str, ServiceTask)>,
) -> Result<(), ServiceError<Error>> {
	let (metrics_iterators, tasks): (HashMap<String, _>, Vec<_>) = items
		.into_iter()
		.filter_map(|(name, task)| {
			let monitor = tokio_metrics::TaskMonitor::new();
			let metrics_iterator = monitor.intervals();
			let task = match task {
				ServiceTask::Optional(true, t) | ServiceTask::Essential(t) =>
					Some(wait_or_shutdown(shutdown_tx.clone(), t)),
				_ => None,
			}?;
			let task = monitor.instrument(task);
			let task = tokio::spawn(task);
			Some(((name.to_string(), metrics_iterator), task))
		})
		.unzip();

	let tokio_metrics = tokio::spawn(wait_or_shutdown(
		shutdown_tx.clone(),
		publish_tokio_metrics(metrics_iterators),
	));

	match join(tokio_metrics, join_all(tasks)).await {
		(Ok(Err(err)), _) => Err(err),
		(_, results) => results
			.into_iter()
			.find(|res| matches!(res, Ok(Err(_))))
			.and_then(|res| res.ok())
			.unwrap_or(Ok(())),
	}
}

type Task = Pin<Box<dyn Future<Output = Result<(), ServiceError<Error>>> + Send + 'static>>;

enum ServiceTask {
	Optional(bool, Task),
	Essential(Task),
}

fn maybe_run<F, E>(should_run: bool, task: F) -> ServiceTask
where
	F: Future<Output = Result<(), E>> + Send + 'static,
	E: Into<ServiceError<Error>>,
{
	ServiceTask::Optional(should_run, Box::pin(task.map_err(|x| x.into())))
}

fn run<F, E>(task: F) -> ServiceTask
where
	F: Future<Output = Result<(), E>> + Send + 'static,
	E: Into<ServiceError<Error>>,
{
	ServiceTask::Essential(Box::pin(task.map_err(|x| x.into())))
}

impl VaultService {
	fn new(
		spacewalk_parachain: SpacewalkParachain,
		config: VaultServiceConfig,
		shutdown: ShutdownSender,
	) -> Self {
		Self { spacewalk_parachain: spacewalk_parachain.clone(), config, shutdown }
	}

	async fn run_service(&self) -> Result<(), ServiceError<Error>> {
		self.await_parachain_block().await?;

		let parsed_auto_register = self
			.config
			.auto_register
			.clone()
			.into_iter()
			.map(|(symbol, amount)| Ok((CurrencyId::try_from_symbol(symbol)?, amount)))
			.into_iter()
			.collect::<Result<Vec<_>, Error>>()
			.map_err(ServiceError::Abort)?;

		// exit if auto-register uses faucet and faucet url not set
		if parsed_auto_register.iter().any(|(_, o)| o.is_none()) && self.config.faucet_url.is_none()
		{
			return Err(ServiceError::Abort(Error::FaucetUrlNotSet))
		}

		// Subscribe to an event (any event will do) so that a period of inactivity does not close
		// the jsonrpsee connection
		let err_provider = self.spacewalk_parachain.clone();
		let err_listener = wait_or_shutdown(self.shutdown.clone(), async move {
			err_provider
				.on_event_error(|e| tracing::debug!("Received error event: {}", e))
				.await?;
			Ok::<_, Error>(())
		});
		tokio::task::spawn(err_listener);

		self.maybe_register_public_key().await?;
		join_all(
			parsed_auto_register
				.iter()
				.map(|(currency_id, amount)| self.maybe_register_vault(currency_id, amount)),
		)
		.await
		.into_iter()
		.collect::<Result<_, Error>>()?;

		// purposefully _after_ maybe_register_vault and _before_ other calls
		self.vault_id_manager.fetch_vault_ids().await?;

		let open_request_executor = execute_open_requests(
			self.shutdown.clone(),
			self.spacewalk_parachain.clone(),
			self.vault_id_manager.clone(),
			self.btc_rpc_master_wallet.clone(),
			self.config.payment_margin_minutes,
			self.config.auto_rbf,
		);
		service::spawn_cancelable(self.shutdown.subscribe(), async move {
			tracing::info!("Checking for open requests...");
			// TODO: kill task on shutdown signal to prevent double payment
			match open_request_executor.await {
				Ok(_) => tracing::info!("Done processing open requests"),
				Err(e) => tracing::error!("Failed to process open requests: {}", e),
			}
		});

		tracing::info!("Starting all services...");
		let tasks = vec![
			(
				"Registered Asset Listener",
				run(listen_for_registered_assets(self.spacewalk_parachain.clone())),
			),
			(
				"Fee Estimate Listener",
				run(listen_for_fee_rate_estimate_changes(self.spacewalk_parachain.clone())),
			),
			(
				"VaultId Registration Listener",
				run(self.vault_id_manager.clone().listen_for_vault_id_registrations()),
			),
			(
				"Restart Timer",
				run(async move {
					tokio::time::sleep(RESTART_INTERVAL).await;
					tracing::info!("Initiating periodic restart...");
					Err(ServiceError::ClientShutdown)
				}),
			),
		];

		run_and_monitor_tasks(self.shutdown.clone(), tasks).await
	}

	async fn maybe_register_public_key(&self) -> Result<(), Error> {
		if let Some(faucet_url) = &self.config.faucet_url {
			// TODO fund account with faucet
		}

		if self.spacewalk_parachain.get_public_key().await?.is_none() {
			tracing::info!("Registering bitcoin public key to the parachain...");
			let public_key = self.btc_rpc_master_wallet.get_new_public_key().await?;
			self.spacewalk_parachain
				.register_public_key(public_key.inner.serialize().into())
				.await?;
		}

		Ok(())
	}

	async fn maybe_register_vault(
		&self,
		collateral_currency: &CurrencyId,
		maybe_collateral_amount: &Option<u128>,
	) -> Result<(), Error> {
		let vault_id = self.get_vault_id(*collateral_currency);

		match is_vault_registered(&self.spacewalk_parachain, &vault_id).await {
			Err(Error::RuntimeError(RuntimeError::VaultLiquidated)) | Ok(true) => {
				tracing::info!(
					"[{}] Not registering vault -- already registered",
					vault_id.pretty_print()
				);
			},
			Ok(false) => {
				tracing::info!("[{}] Not registered", vault_id.pretty_print());
				if let Some(collateral) = maybe_collateral_amount {
					tracing::info!("[{}] Automatically registering...", vault_id.pretty_print());
					let free_balance = self
						.spacewalk_parachain
						.get_free_balance(vault_id.collateral_currency())
						.await?;
					self.spacewalk_parachain
						.register_vault(
							&vault_id,
							if collateral.gt(&free_balance) {
								tracing::warn!(
									"Cannot register with {}, using the available free balance: {}",
									collateral,
									free_balance
								);
								free_balance
							} else {
								*collateral
							},
						)
						.await?;
				} else if let Some(faucet_url) = &self.config.faucet_url {
					tracing::info!("[{}] Automatically registering...", vault_id.pretty_print());
					// TODO
					// faucet::fund_and_register(&self.spacewalk_parachain, faucet_url, &vault_id)
					// 	.await?;
				}
			},
			Err(x) => return Err(x),
		}

		Ok(())
	}

	async fn await_parachain_block(&self) -> Result<u32, Error> {
		// wait for a new block to arrive, to prevent processing an event that potentially
		// has been processed already prior to restarting
		tracing::info!("Waiting for new block...");
		let startup_height = self.spacewalk_parachain.get_current_chain_height().await?;
		while startup_height == self.spacewalk_parachain.get_current_chain_height().await? {
			sleep(CHAIN_HEIGHT_POLLING_INTERVAL).await;
		}
		tracing::info!("Got new block...");
		Ok(startup_height)
	}
}

pub(crate) async fn is_vault_registered(
	parachain_rpc: &SpacewalkParachain,
	vault_id: &VaultId,
) -> Result<bool, Error> {
	match parachain_rpc.get_vault(vault_id).await {
		Ok(_) => Ok(true),
		Err(RuntimeError::VaultNotFound) => Ok(false),
		Err(err) => Err(err.into()),
	}
}
