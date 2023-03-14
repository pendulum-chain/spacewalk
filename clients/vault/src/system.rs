use std::{
	collections::HashMap, convert::TryInto, fs, future::Future, pin::Pin, str::from_utf8,
	sync::Arc, time::Duration,
};

use async_trait::async_trait;
use clap::Parser;
use futures::{
	channel::{mpsc, mpsc::Sender},
	future::join_all,
	SinkExt, TryFutureExt,
};
use git_version::git_version;
use tokio::{sync::RwLock, time::sleep};

use runtime::{
	cli::parse_duration_minutes, CollateralBalancesPallet, CurrencyId, Error as RuntimeError,
	IssueRequestsMap, PrettyPrint, RegisterVaultEvent, ShutdownSender, SpacewalkParachain,
	StellarRelayPallet, TryFromSymbol, UpdateActiveBlockEvent, UtilFuncs, VaultCurrencyPair,
	VaultId, VaultRegistryPallet,
};
use service::{wait_or_shutdown, Error as ServiceError, Service};
use wallet::{LedgerTxEnvMap, StellarWallet};

use crate::{
	cancellation::ReplaceCanceller,
	error::Error,
	execution::execute_open_requests,
	issue,
	issue::IssueFilter,
	oracle::OracleAgent,
	redeem::listen_for_redeem_requests,
	replace::{listen_for_accept_replace, listen_for_execute_replace, listen_for_replace_requests},
	service::{CancellationScheduler, IssueCanceller},
	ArcRwLock, Event, CHAIN_HEIGHT_POLLING_INTERVAL,
};

pub const VERSION: &str = git_version!(args = ["--tags"], fallback = "unknown");
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

const RESTART_INTERVAL: Duration = Duration::from_secs(10800); // restart every 3 hours

#[derive(Clone)]
pub struct VaultData {
	pub vault_id: VaultId,
	pub stellar_wallet: ArcRwLock<StellarWallet>,
}

#[derive(Clone)]
pub struct VaultIdManager {
	vault_data: ArcRwLock<HashMap<VaultId, VaultData>>,
	spacewalk_parachain: SpacewalkParachain,
	stellar_wallet: ArcRwLock<StellarWallet>,
}

impl VaultIdManager {
	pub fn new(
		spacewalk_parachain: SpacewalkParachain,
		stellar_wallet: ArcRwLock<StellarWallet>,
	) -> Self {
		Self {
			vault_data: Arc::new(RwLock::new(HashMap::new())),
			spacewalk_parachain,
			stellar_wallet,
		}
	}

	// used for testing only
	pub fn from_map(
		spacewalk_parachain: SpacewalkParachain,
		stellar_wallet: ArcRwLock<StellarWallet>,
		vault_ids: Vec<VaultId>,
	) -> Self {
		let vault_data = vault_ids
			.iter()
			.map(|key| {
				(
					key.clone(),
					VaultData { vault_id: key.clone(), stellar_wallet: stellar_wallet.clone() },
				)
			})
			.collect();
		Self { vault_data: Arc::new(RwLock::new(vault_data)), spacewalk_parachain, stellar_wallet }
	}

	async fn add_vault_id(&self, vault_id: VaultId) -> Result<(), Error> {
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

	pub async fn get_stellar_wallet(&self, vault_id: &VaultId) -> Option<ArcRwLock<StellarWallet>> {
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

	// This could be refactored since at the moment every vault has the same stellar wallet. But
	// we might want to change that in the future
	pub async fn get_vault_stellar_wallets(&self) -> Vec<(VaultId, ArcRwLock<StellarWallet>)> {
		self.vault_data
			.read()
			.await
			.iter()
			.map(|(vault_id, data)| (vault_id.clone(), data.stellar_wallet.clone()))
			.collect()
	}
}

/// Expecting an input of the form: `collateral_currency,wrapped_currency,collateral_amount` with
/// `collateral_currency` being the currency of the collateral (e.g. DOT, KSM, ...),
/// `wrapped_currency` being the currency codes of the wrapped currency (e.g. USDC, EURT...)
///  including the issuer and code, ie 'GABC...:USDC'  and
/// `collateral_amount` being the amount of collateral to be locked.
#[allow(clippy::format_in_format_args, clippy::useless_conversion)]
fn parse_collateral_and_amount(
	s: &str,
) -> Result<(String, String, Option<u128>), Box<dyn std::error::Error + Send + Sync + 'static>> {
	let parts: Vec<&str> = s
		.split(',')
		.map(|s| s.trim())
		.collect::<Vec<_>>()
		.try_into()
		.map_err(|_| format!("invalid auto-register parameter: `{}`", s))?;

	assert_eq!(
		parts.len(),
		3,
		"{}",
		format!("invalid string, expected 3 parts, got {} for `{}`", parts.len(), s)
	);

	let collateral_currency = parts[0].to_string();
	let wrapped_currency = parts[1].to_string();
	let collateral_amount = parts[2].parse::<u128>()?;
	Ok((collateral_currency, wrapped_currency, Some(collateral_amount)))
}

#[derive(Parser, Clone, Debug)]
pub struct VaultServiceConfig {
	#[clap(long, help = "The Stellar secret key that is used to sign transactions.")]
	pub stellar_vault_secret_key_filepath: String,

	/// Pass the faucet URL for auto-registration.
	#[clap(long)]
	pub faucet_url: Option<String>,

	/// Automatically register the vault with the given amount of collateral
	#[clap(long, value_parser = parse_collateral_and_amount)]
	pub auto_register: Vec<(String, String, Option<u128>)>,

	/// Minimum time to the the redeem/replace execution deadline to make the stellar payment.
	#[clap(long, value_parser = parse_duration_minutes, default_value = "1")]
	pub payment_margin_minutes: Duration,

	/// Opt out of participation in replace requests.
	#[clap(long)]
	pub no_auto_replace: bool,

	/// Don't try to execute issues.
	#[clap(long)]
	pub no_issue_execution: bool,
}

async fn active_block_listener(
	parachain_rpc: SpacewalkParachain,
	issue_tx: Sender<Event>,
	replace_tx: Sender<Event>,
) -> Result<(), ServiceError<Error>> {
	let issue_tx = &issue_tx;
	let replace_tx = &replace_tx;
	parachain_rpc
		.on_event::<UpdateActiveBlockEvent, _, _, _>(
			|event| async move {
				let _ = issue_tx.clone().send(Event::ParachainBlock(event.block_number)).await;
				let _ = replace_tx.clone().send(Event::ParachainBlock(event.block_number)).await;
			},
			|err| tracing::error!("Error (UpdateActiveBlockEvent): {}", err.to_string()),
		)
		.await?;
	Ok(())
}

pub struct VaultService {
	spacewalk_parachain: SpacewalkParachain,
	stellar_wallet: ArcRwLock<StellarWallet>,
	config: VaultServiceConfig,
	shutdown: ShutdownSender,
	vault_id_manager: VaultIdManager,
}

#[async_trait]
impl Service<VaultServiceConfig, Error> for VaultService {
	const NAME: &'static str = NAME;
	const VERSION: &'static str = VERSION;

	async fn new_service(
		spacewalk_parachain: SpacewalkParachain,
		config: VaultServiceConfig,
		shutdown: ShutdownSender,
	) -> Result<Self, Error> {
		VaultService::new(spacewalk_parachain, config, shutdown).await
	}

	async fn start(&mut self) -> Result<(), ServiceError<Error>> {
		self.run_service().await
	}
}

async fn run_and_monitor_tasks(
	shutdown_tx: ShutdownSender,
	items: Vec<(&str, ServiceTask)>,
) -> Result<(), ServiceError<Error>> {
	let (_metrics_iterators, tasks): (HashMap<String, _>, Vec<_>) = items
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

	// We don't use metrics for now
	// let tokio_metrics = tokio::spawn(wait_or_shutdown(
	// 	shutdown_tx.clone(),
	// 	publish_tokio_metrics(metrics_iterators),
	// ));

	let results = join_all(tasks).await;
	results
		.into_iter()
		.find(|res| matches!(res, Ok(Err(_))))
		.and_then(|res| res.ok())
		.unwrap_or(Ok(()))
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
	async fn new(
		spacewalk_parachain: SpacewalkParachain,
		config: VaultServiceConfig,
		shutdown: ShutdownSender,
	) -> Result<Self, Error> {
		let is_public_network = spacewalk_parachain.is_public_network().await;
		let is_public_network = match is_public_network {
			Ok(is_public_network) => is_public_network,
			Err(error) => {
				// Sometimes the fetch fails with 'StorageItemNotFound' error.
				// We assume public network by default
				tracing::warn!(
					"Failed to fetch public network status from parachain: {}. Assuming public network.",
					error
				);
				true
			},
		};

		let stellar_vault_secret_key =
			fs::read_to_string(&config.stellar_vault_secret_key_filepath)?;
		let stellar_wallet = StellarWallet::from_secret_encoded(
			&stellar_vault_secret_key.trim().to_string(),
			is_public_network,
		)?;
		tracing::debug!(
			"Vault wallet public key: {}",
			from_utf8(&stellar_wallet.get_public_key().to_encoding())?
		);

		let stellar_wallet = Arc::new(RwLock::new(stellar_wallet));

		Ok(Self {
			spacewalk_parachain: spacewalk_parachain.clone(),
			stellar_wallet: stellar_wallet.clone(),
			config,
			shutdown,
			vault_id_manager: VaultIdManager::new(spacewalk_parachain, stellar_wallet),
		})
	}

	fn get_vault_id(
		&self,
		collateral_currency: CurrencyId,
		wrapped_currency: CurrencyId,
	) -> VaultId {
		let account_id = self.spacewalk_parachain.get_account_id();

		VaultId {
			account_id: account_id.clone(),
			currencies: VaultCurrencyPair {
				collateral: collateral_currency,
				wrapped: wrapped_currency,
			},
		}
	}

	async fn run_service(&mut self) -> Result<(), ServiceError<Error>> {
		let startup_height = self.await_parachain_block().await?;
		let account_id = self.spacewalk_parachain.get_account_id().clone();

		let parsed_auto_register = self
			.config
			.auto_register
			.clone()
			.into_iter()
			.map(|(collateral, wrapped, amount)| {
				Ok((
					CurrencyId::try_from_symbol(collateral)?,
					CurrencyId::try_from_symbol(wrapped)?,
					amount,
				))
			})
			.into_iter()
			.collect::<Result<Vec<_>, Error>>()
			.map_err(ServiceError::Abort)?;

		// exit if auto-register uses faucet and faucet url not set
		if parsed_auto_register.iter().any(|(_, _, o)| o.is_none()) &&
			self.config.faucet_url.is_none()
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
		join_all(parsed_auto_register.iter().map(
			|(collateral_currency, wrapped_currency, amount)| {
				self.maybe_register_vault(collateral_currency, wrapped_currency, amount)
			},
		))
		.await
		.into_iter()
		.collect::<Result<_, Error>>()?;

		// purposefully _after_ maybe_register_vault and _before_ other calls
		self.vault_id_manager.fetch_vault_ids().await?;

		let wallet = self.stellar_wallet.read().await;
		let vault_public_key = wallet.get_public_key();
		let is_public_network = wallet.is_public_network();
		drop(wallet);

		let mut oracle_agent =
			OracleAgent::new(is_public_network).expect("Failed to create oracle agent");
		oracle_agent.start().await.expect("Failed to start oracle agent");
		let oracle_agent = Arc::new(oracle_agent);

		let open_request_executor = execute_open_requests(
			self.shutdown.clone(),
			self.spacewalk_parachain.clone(),
			self.vault_id_manager.clone(),
			self.stellar_wallet.clone(),
			oracle_agent.clone(),
			self.config.payment_margin_minutes,
		);
		service::spawn_cancelable(self.shutdown.subscribe(), async move {
			tracing::info!("Checking for open requests...");
			// TODO: kill task on shutdown signal to prevent double payment
			match open_request_executor.await {
				Ok(_) => tracing::info!("Done processing open requests"),
				Err(e) => tracing::error!("Failed to process open requests: {}", e),
			}
		});

		// issue handling
		// this vec is passed to the stellar wallet to filter out transactions that are not relevant
		// this has to be modified every time the issue set changes
		let issue_map: ArcRwLock<IssueRequestsMap> = Arc::new(RwLock::new(IssueRequestsMap::new()));
		issue::initialize_issue_set(&self.spacewalk_parachain, &issue_map).await?;

		let issue_filter = IssueFilter::new(&vault_public_key)?;

		let ledger_env_map: ArcRwLock<LedgerTxEnvMap> = Arc::new(RwLock::new(HashMap::new()));

		let (issue_event_tx, issue_event_rx) = mpsc::channel::<Event>(32);
		let (replace_event_tx, replace_event_rx) = mpsc::channel::<Event>(16);

		tracing::info!("Starting all services...");
		let tasks = vec![
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
			(
				"Stellar Transaction Listener",
				run(wallet::listen_for_new_transactions(
					vault_public_key.clone(),
					is_public_network,
					ledger_env_map.clone(),
					issue_map.clone(),
					issue_filter,
				)),
			),
			(
				"Issue Request Listener",
				run(issue::listen_for_issue_requests(
					self.spacewalk_parachain.clone(),
					vault_public_key,
					issue_event_tx.clone(),
					issue_map.clone(),
				)),
			),
			(
				"Issue Cancel Listener",
				run(issue::listen_for_issue_cancels(
					self.spacewalk_parachain.clone(),
					issue_map.clone(),
				)),
			),
			(
				"Issue Execute Listener",
				run(issue::listen_for_executed_issues(
					self.spacewalk_parachain.clone(),
					issue_map.clone(),
				)),
			),
			(
				"Issue Executor",
				maybe_run(
					!self.config.no_issue_execution,
					issue::process_issues_requests(
						self.spacewalk_parachain.clone(),
						oracle_agent.clone(),
						ledger_env_map.clone(),
						issue_map.clone(),
					),
				),
			),
			(
				"Issue Cancel Scheduler",
				run(CancellationScheduler::new(
					self.spacewalk_parachain.clone(),
					startup_height,
					account_id.clone(),
				)
				.handle_cancellation::<IssueCanceller>(issue_event_rx)),
			),
			(
				"Request Replace Listener",
				run(listen_for_replace_requests(
					self.spacewalk_parachain.clone(),
					self.vault_id_manager.clone(),
					replace_event_tx.clone(),
					!self.config.no_auto_replace,
				)),
			),
			(
				"Accept Replace Listener",
				run(listen_for_accept_replace(
					self.shutdown.clone(),
					self.spacewalk_parachain.clone(),
					self.vault_id_manager.clone(),
					self.config.payment_margin_minutes,
					oracle_agent.clone(),
				)),
			),
			(
				"Execute Replace Listener",
				run(listen_for_execute_replace(
					self.spacewalk_parachain.clone(),
					replace_event_tx.clone(),
				)),
			),
			(
				"Replace Cancellation Scheduler",
				run(CancellationScheduler::new(
					self.spacewalk_parachain.clone(),
					startup_height,
					account_id.clone(),
				)
				.handle_cancellation::<ReplaceCanceller>(replace_event_rx)),
			),
			(
				"Parachain Block Listener",
				run(active_block_listener(
					self.spacewalk_parachain.clone(),
					issue_event_tx.clone(),
					replace_event_tx.clone(),
				)),
			),
			(
				"Redeem Request Listener",
				run(listen_for_redeem_requests(
					self.shutdown.clone(),
					self.spacewalk_parachain.clone(),
					self.vault_id_manager.clone(),
					self.config.payment_margin_minutes,
					oracle_agent.clone(),
				)),
			),
		];

		run_and_monitor_tasks(self.shutdown.clone(), tasks).await
	}

	async fn maybe_register_public_key(&mut self) -> Result<(), Error> {
		if let Some(_faucet_url) = &self.config.faucet_url {
			// TODO fund account with faucet
		}

		if self.spacewalk_parachain.get_public_key().await?.is_none() {
			let public_key = self.stellar_wallet.read().await.get_public_key_raw();
			tracing::info!("Registering public key to the parachain... {:?}", public_key);
			self.spacewalk_parachain.register_public_key(public_key).await?;
		} else {
			tracing::info!("Public key already registered");
		}

		Ok(())
	}

	async fn maybe_register_vault(
		&self,
		collateral_currency: &CurrencyId,
		wrapped_currency: &CurrencyId,
		maybe_collateral_amount: &Option<u128>,
	) -> Result<(), Error> {
		let vault_id = self.get_vault_id(*collateral_currency, *wrapped_currency);

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
				} else if let Some(_faucet_url) = &self.config.faucet_url {
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
