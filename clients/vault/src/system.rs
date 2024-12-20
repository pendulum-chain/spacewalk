#![allow(clippy::too_many_arguments)]
use std::{
	collections::HashMap, convert::TryInto, fs, future::Future, pin::Pin, str::from_utf8,
	sync::Arc, time::Duration,
};

use async_trait::async_trait;
use clap::Parser;
use futures::{
	channel::{
		mpsc,
		mpsc::{Receiver as mpscReceiver, Sender as mpscSender},
	},
	future::{join, join_all},
	SinkExt, TryFutureExt,
};
use tokio::{
	sync::{
		broadcast::{self, Receiver as bcReceiver, Sender as bcSender},
		RwLock,
	},
	time::sleep,
};

use runtime::{
	cli::parse_duration_minutes, AccountId, BlockNumber, CollateralBalancesPallet, CurrencyId,
	Error as RuntimeError, IssueIdLookup, IssueRequestsMap, PrettyPrint, RegisterVaultEvent,
	ShutdownSender, SpacewalkParachain, StellarRelayPallet, TryFromSymbol, UpdateActiveBlockEvent,
	UtilFuncs, VaultCurrencyPair, VaultId, VaultRegistryPallet,
};
use service::{wait_or_shutdown, Error as ServiceError, MonitoringConfig, Service};
use stellar_relay_lib::{sdk::PublicKey, StellarOverlayConfig};
use wallet::{LedgerTxEnvMap, StellarWallet, RESUBMISSION_INTERVAL_IN_SECS};

use crate::{
	cancellation::ReplaceCanceller,
	error::Error,
	issue,
	issue::IssueFilter,
	metrics::{monitor_bridge_metrics, poll_metrics, publish_tokio_metrics, PerCurrencyMetrics},
	oracle::{listen_for_stellar_messages, OracleAgent},
	redeem::listen_for_redeem_requests,
	replace::{listen_for_accept_replace, listen_for_execute_replace, listen_for_replace_requests},
	requests::execution::execute_open_requests,
	service::{CancellationScheduler, IssueCanceller},
	tokio_spawn, ArcRwLock, Event, CHAIN_HEIGHT_POLLING_INTERVAL,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

const RESTART_INTERVAL: Duration = Duration::from_secs(7200); // restart every 2 hours

#[derive(Clone, Debug)]
pub struct VaultData {
	pub vault_id: VaultId,
	pub stellar_wallet: ArcRwLock<StellarWallet>,
	pub metrics: PerCurrencyMetrics,
	pub liquidated: bool,
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
					VaultData {
						vault_id: key.clone(),
						stellar_wallet: stellar_wallet.clone(),
						metrics: PerCurrencyMetrics::dummy(),
						liquidated: false,
					},
				)
			})
			.collect();
		Self { vault_data: Arc::new(RwLock::new(vault_data)), spacewalk_parachain, stellar_wallet }
	}

	async fn add_vault_id(&self, vault_id: VaultId, is_liquidated: bool) -> Result<(), Error> {
		let metrics = PerCurrencyMetrics::new(&vault_id);
		let data = VaultData {
			vault_id: vault_id.clone(),
			stellar_wallet: self.stellar_wallet.clone(),
			metrics,
			liquidated: is_liquidated,
		};
		PerCurrencyMetrics::initialize_values(self.spacewalk_parachain.clone(), &data).await;

		tracing::info!("Adding vault with ID: {vault_id:?}");

		self.vault_data.write().await.insert(vault_id, data.clone());

		Ok(())
	}

	pub async fn fetch_vault_ids(&self) -> Result<(), Error> {
		for vault_id in self
			.spacewalk_parachain
			.get_vaults_by_account_id(self.spacewalk_parachain.get_account_id())
			.await?
		{
			// check if vault is registered
			match self.spacewalk_parachain.get_vault(&vault_id).await {
				Ok(_) => self.add_vault_id(vault_id.clone(), false).await?,
				Err(RuntimeError::VaultLiquidated) => {
					self.add_vault_id(vault_id.clone(), true).await?;
					tracing::error!(
						"[{}] Vault is liquidated -- not going to process events for this vault.",
						vault_id.pretty_print()
					);
				},
				Err(e) => return Err(e.into()),
			}
		}
		Ok(())
	}

	pub async fn listen_for_vault_id_registrations(self) -> Result<(), ServiceError<Error>> {
		tracing::info!("listen_for_vault_id_registrations(): started");
		Ok(self
			.spacewalk_parachain
			.on_event::<RegisterVaultEvent, _, _, _>(
				|event| async {
					let vault_id = event.vault_id;
					if self.spacewalk_parachain.is_this_vault(&vault_id) {
						tracing::info!("New vault registered: {}", vault_id.pretty_print());
						let _ = self.add_vault_id(vault_id, false).await;
					}
				},
				|err| tracing::error!("Error (RegisterVaultEvent): {}", err.to_string()),
			)
			.await?)
	}

	pub async fn get_stellar_wallet(&self, vault_id: &VaultId) -> Option<ArcRwLock<StellarWallet>> {
		self.vault_data.read().await.get(vault_id).map(|x| x.stellar_wallet.clone())
	}

	pub async fn get_active_vault(&self, vault_id: &VaultId) -> Option<VaultData> {
		let vault = self.vault_data.read().await.get(vault_id)?.clone();
		// Filter liquidated
		if vault.liquidated {
			return None;
		}
		return Some(vault);
	}

	pub async fn get_vault(&self, vault_id: &VaultId) -> Option<VaultData> {
		self.vault_data.read().await.get(vault_id).cloned()
	}

	// Get all ACTIVE vaults
	pub async fn get_entries(&self) -> Vec<VaultData> {
		self.vault_data
			.read()
			.await
			.iter()
			.filter(|(_, value)| !value.liquidated)
			.map(|(_, value)| value.clone())
			.collect()
	}

	// Get all vaults including liquidated ones.
	pub async fn get_all_entries(&self) -> Vec<VaultData> {
		self.vault_data.read().await.iter().map(|(_, value)| value.clone()).collect()
	}

	pub async fn get_vault_ids(&self) -> Vec<VaultId> {
		self.vault_data
			.read()
			.await
			.iter()
			.filter(|(_, value)| !value.liquidated)
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
			.filter(|(_, value)| !value.liquidated)
			.map(|(vault_id, data)| (vault_id.clone(), data.stellar_wallet.clone()))
			.collect()
	}
}

/// Expecting an input of the form: `collateral_currency,wrapped_currency,collateral_amount` with
/// `collateral_currency` being the XCM index of the currency to be locked (e.g. 0, 1, 2...),
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
	#[clap(
		long,
		env = "STELLAR_VAULT_SECRET_KEY_FILEPATH",
		help = "The Stellar secret key that is used to sign transactions."
	)]
	pub stellar_vault_secret_key_filepath: String,

	#[clap(
		long,
		env = "STELLAR_OVERLAY_CONFIG_FILEPATH",
		help = "The filepath where the json config for StellarOverlay is located"
	)]
	pub stellar_overlay_config_filepath: String,

	/// Pass the faucet URL for auto-registration.
	#[clap(long, env = "FAUCET_URL")]
	pub faucet_url: Option<String>,

	/// Automatically register the vault with the given amount of collateral
	/// note: when specifying the env, make sure to enclose it with double quotes.
	#[clap(long, env = "AUTO_REGISTER", value_parser = parse_collateral_and_amount)]
	pub auto_register: Vec<(String, String, Option<u128>)>,

	/// Minimum time to the redeem/replace execution deadline to make the stellar payment.
	#[clap(long, env = "PAYMENT_MARGIN_MINUTES", value_parser = parse_duration_minutes, default_value = "1")]
	pub payment_margin_minutes: Duration,

	/// Opt out of participation in replace requests.
	#[clap(long, env = "NO_AUTO_REPLACE")]
	pub no_auto_replace: bool,

	/// Don't try to execute issues.
	#[clap(long, env = "NO_ISSUE_EXECUTION")]
	pub no_issue_execution: bool,
}

async fn active_block_listener(
	parachain_rpc: SpacewalkParachain,
	issue_tx: mpscSender<Event>,
	replace_tx: mpscSender<Event>,
) -> Result<(), ServiceError<Error>> {
	tracing::info!("active_block_listener(): started");
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

	tracing::info!("active_block_listener(): ended");
	Ok(())
}

pub struct VaultService {
	spacewalk_parachain: SpacewalkParachain,
	stellar_wallet: ArcRwLock<StellarWallet>,
	config: VaultServiceConfig,
	monitoring_config: MonitoringConfig,
	shutdown: ShutdownSender,
	vault_id_manager: VaultIdManager,
	secret_key: String,
	agent: Option<Arc<OracleAgent>>,
}

#[async_trait]
impl Service<VaultServiceConfig, Error> for VaultService {
	const NAME: &'static str = NAME;
	const VERSION: &'static str = VERSION;

	async fn new_service(
		spacewalk_parachain: SpacewalkParachain,
		config: VaultServiceConfig,
		monitoring_config: MonitoringConfig,
		shutdown: ShutdownSender,
	) -> Result<Self, Error> {
		VaultService::new(spacewalk_parachain, config, monitoring_config, shutdown).await
	}

	async fn start(&mut self) -> Result<(), ServiceError<Error>> {
		let result = self.run_service().await;

		self.try_shutdown_wallet().await;

		if let Err(error) = result {
			tracing::error!("start(): Failed to run service: {error:?}");
			let _ = self.shutdown.send(());
			Err(error)
		} else {
			result
		}
	}
}

async fn run_and_monitor_tasks(
	shutdown_tx: ShutdownSender,
	items: Vec<(&str, ServiceTask)>,
	// Sends a signal to start those tasks requiring a precheck.
	mut precheck_signals: Vec<bcReceiver<()>>,
) -> Result<(), ServiceError<Error>> {
	let (metrics_iterators, tasks): (HashMap<String, _>, Vec<_>) = items
		.into_iter()
		.filter_map(|(name, task)| {
			let monitor = tokio_metrics::TaskMonitor::new();
			let metrics_iterator = monitor.intervals();
			let task = match task {
				ServiceTask::Optional(true, t) | ServiceTask::Essential(t) =>
					Some(wait_or_shutdown(shutdown_tx.clone(), t, None)),
				ServiceTask::PrecheckRequired(t) =>
					Some(wait_or_shutdown(shutdown_tx.clone(), t, precheck_signals.pop())),
				_ => None,
			}?;
			let task = monitor.instrument(task);

			let task = tokio_spawn(name, task);
			Some(((name.to_string(), metrics_iterator), task))
		})
		.unzip();

	let tokio_metrics = tokio_spawn(
		"tokio metrics publisher",
		wait_or_shutdown(shutdown_tx.clone(), publish_tokio_metrics(metrics_iterators), None),
	);

	tracing::info!("run_and_monitor_tasks(): running all tasks...");

	match join(tokio_metrics, join_all(tasks)).await {
		(Ok(Err(err)), _) => Err(err),
		(_, results) => {
			for result in results {
				match result {
					Ok(_) => {},
					Err(e) => {
						tracing::error!("run_and_monitor_tasks(): One of the tasks failed: {e:?}");
						return Err(ServiceError::TokioError(e));
					},
				}
			}
			Ok(())
		},
	}
}

type Task = Pin<Box<dyn Future<Output = Result<(), ServiceError<Error>>> + Send + 'static>>;

enum ServiceTask {
	Optional(bool, Task),
	Essential(Task),
	// Runs a task after a prequisite check has passed.
	PrecheckRequired(Task),
}

/// returns a single-producer multi-consumer channel to send a signal to start a task
fn precheck_signals(num_of_signals_required: u8) -> (bcSender<()>, Vec<bcReceiver<()>>) {
	let (sender, receiver) = broadcast::channel(1);
	let mut subscribers = vec![receiver];
	while subscribers.len() < usize::from(num_of_signals_required) {
		subscribers.push(sender.subscribe());
	}

	(sender, subscribers)
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

fn run_with_precheck<F, E>(task: F) -> ServiceTask
where
	F: Future<Output = Result<(), E>> + Send + 'static,
	E: Into<ServiceError<Error>>,
{
	ServiceTask::PrecheckRequired(Box::pin(task.map_err(|x| x.into())))
}

type RegistrationData = Vec<(CurrencyId, CurrencyId, Option<u128>)>;
// dedicated for running the service
impl VaultService {
	fn auto_register(&self) -> Result<RegistrationData, ServiceError<Error>> {
		let mut amount_is_none: bool = false;
		let parsed_auto_register = self
			.config
			.auto_register
			.clone()
			.into_iter()
			.map(|(collateral, wrapped, amount)| {
				if amount.is_none() {
					amount_is_none = true;
				}

				Ok((
					CurrencyId::try_from_symbol(collateral)?,
					CurrencyId::try_from_symbol(wrapped)?,
					amount,
				))
			})
			.collect::<Result<Vec<_>, Error>>()
			.map_err(ServiceError::Abort)?;

		// exit if auto-register uses faucet and faucet url not set
		if amount_is_none && self.config.faucet_url.is_none() {
			return Err(ServiceError::Abort(Error::FaucetUrlNotSet));
		}

		Ok(parsed_auto_register)
	}

	fn maintain_connection(&self) -> Result<(), ServiceError<Error>> {
		// Subscribe to an event (any event will do) so that a period of inactivity does not close
		// the jsonrpsee connection
		let err_provider = self.spacewalk_parachain.clone();
		let err_listener = wait_or_shutdown(
			self.shutdown.clone(),
			async move {
				err_provider
					.on_event_error(|e| {
						tracing::debug!("Client Service: Received error event: {}", e)
					})
					.await?;
				Ok::<_, Error>(())
			},
			None,
		);
		tokio_spawn("error listener", err_listener);

		Ok(())
	}

	async fn create_oracle_agent(
		&self,
		is_public_network: bool,
		shutdown_sender: ShutdownSender,
	) -> Result<Arc<OracleAgent>, ServiceError<Error>> {
		let stellar_overlay_cfg = self.stellar_overlay_cfg()?;

		// check if both the config file and the wallet are the same.
		if is_public_network != stellar_overlay_cfg.is_public_network() {
			return Err(ServiceError::IncompatibleNetwork);
		}

		let oracle_agent =
			OracleAgent::new(&stellar_overlay_cfg, self.secret_key(), shutdown_sender)
				.await
				.map_err(|e| {
					tracing::error!("Failed to create OracleAgent: {e:?}");
					ServiceError::OracleError(Error::OracleError(e))
				})?;

		Ok(Arc::new(oracle_agent))
	}

	fn create_issue_tasks(
		&self,
		issue_event_tx: mpscSender<Event>,
		issue_event_rx: mpscReceiver<Event>,
		startup_height: BlockNumber,
		account_id: AccountId,
		vault_public_key: PublicKey,
		oracle_agent: Arc<OracleAgent>,
		issue_map: ArcRwLock<IssueRequestsMap>,
		ledger_env_map: ArcRwLock<LedgerTxEnvMap>,
		memos_to_issue_ids: ArcRwLock<IssueIdLookup>,
	) -> Vec<(&str, ServiceTask)> {
		vec![
			(
				"Issue Request Listener",
				run_with_precheck(issue::listen_for_issue_requests(
					self.spacewalk_parachain.clone(),
					vault_public_key,
					issue_event_tx,
					issue_map.clone(),
					memos_to_issue_ids.clone(),
				)),
			),
			(
				"Issue Cancel Listener",
				run_with_precheck(issue::listen_for_issue_cancels(
					self.spacewalk_parachain.clone(),
					issue_map.clone(),
					memos_to_issue_ids.clone(),
				)),
			),
			(
				"Issue Execute Listener",
				run_with_precheck(issue::listen_for_executed_issues(
					self.spacewalk_parachain.clone(),
					issue_map.clone(),
					memos_to_issue_ids.clone(),
				)),
			),
			(
				"Issue Executor",
				maybe_run(
					!self.config.no_issue_execution,
					issue::process_issues_requests(
						self.spacewalk_parachain.clone(),
						oracle_agent,
						ledger_env_map,
						issue_map,
						memos_to_issue_ids,
					),
				),
			),
			(
				"Issue Cancel Scheduler",
				run_with_precheck(
					CancellationScheduler::new(
						self.spacewalk_parachain.clone(),
						startup_height,
						account_id,
					)
					.handle_cancellation::<IssueCanceller>(issue_event_rx),
				),
			),
		]
	}

	fn create_replace_tasks(
		&self,
		replace_event_tx: mpscSender<Event>,
		replace_event_rx: mpscReceiver<Event>,
		startup_height: BlockNumber,
		account_id: AccountId,
		oracle_agent: Arc<OracleAgent>,
	) -> Vec<(&str, ServiceTask)> {
		vec![
			(
				"Request Replace Listener",
				run_with_precheck(listen_for_replace_requests(
					self.spacewalk_parachain.clone(),
					self.vault_id_manager.clone(),
					replace_event_tx.clone(),
					!self.config.no_auto_replace,
				)),
			),
			(
				"Accept Replace Listener",
				run_with_precheck(listen_for_accept_replace(
					self.shutdown.clone(),
					self.spacewalk_parachain.clone(),
					self.vault_id_manager.clone(),
					self.config.payment_margin_minutes,
					oracle_agent,
				)),
			),
			(
				"Execute Replace Listener",
				run(listen_for_execute_replace(self.spacewalk_parachain.clone(), replace_event_tx)),
			),
			(
				"Replace Cancellation Scheduler",
				run_with_precheck(
					CancellationScheduler::new(
						self.spacewalk_parachain.clone(),
						startup_height,
						account_id,
					)
					.handle_cancellation::<ReplaceCanceller>(replace_event_rx),
				),
			),
		]
	}

	fn create_bridge_metrics_tasks(&self) -> Vec<(&str, ServiceTask)> {
		vec![
			(
				"Bridge Metrics Listener",
				maybe_run(
					!self.monitoring_config.no_prometheus,
					monitor_bridge_metrics(
						self.spacewalk_parachain.clone(),
						self.vault_id_manager.clone(),
					),
				),
			),
			(
				"Bridge Metrics Poller",
				maybe_run(
					!self.monitoring_config.no_prometheus,
					poll_metrics(self.spacewalk_parachain.clone(), self.vault_id_manager.clone()),
				),
			),
		]
	}

	fn create_initial_tasks(
		&self,
		is_public_network: bool,
		issue_event_tx: mpscSender<Event>,
		replace_event_tx: mpscSender<Event>,
		vault_public_key: PublicKey,
		issue_map: ArcRwLock<IssueRequestsMap>,
		ledger_env_map: ArcRwLock<LedgerTxEnvMap>,
		memos_to_issue_ids: ArcRwLock<IssueIdLookup>,
	) -> Result<Vec<(&str, ServiceTask)>, ServiceError<Error>> {
		let issue_filter = IssueFilter::new(&vault_public_key)?;

		Ok(vec![
			(
				"VaultId Registration Listener",
				run(self.vault_id_manager.clone().listen_for_vault_id_registrations()),
			),
			(
				"Restart Timer",
				run(async move {
					tracing::info!(
						"Periodic restart in {:?} minutes.",
						RESTART_INTERVAL.as_secs().checked_div(60).unwrap_or(0)
					);
					tokio::time::sleep(RESTART_INTERVAL).await;
					tracing::info!("Initiating periodic restart...");
					Err(ServiceError::ClientShutdown)
				}),
			),
			(
				"Stellar Transaction Listener",
				run(wallet::listen_for_new_transactions(
					vault_public_key,
					is_public_network,
					ledger_env_map,
					issue_map,
					memos_to_issue_ids,
					issue_filter,
				)),
			),
			(
				"Parachain Block Listener",
				run_with_precheck(active_block_listener(
					self.spacewalk_parachain.clone(),
					issue_event_tx,
					replace_event_tx,
				)),
			),
		])
	}

	fn create_tasks(
		&self,
		startup_height: BlockNumber,
		account_id: AccountId,
		is_public_network: bool,
		vault_public_key: PublicKey,
		oracle_agent: Arc<OracleAgent>,
		issue_map: ArcRwLock<IssueRequestsMap>,
		ledger_env_map: ArcRwLock<LedgerTxEnvMap>,
		memos_to_issue_ids: ArcRwLock<IssueIdLookup>,
	) -> Result<Vec<(&str, ServiceTask)>, ServiceError<Error>> {
		let (issue_event_tx, issue_event_rx) = mpsc::channel::<Event>(32);
		let (replace_event_tx, replace_event_rx) = mpsc::channel::<Event>(16);

		let mut tasks = self.create_initial_tasks(
			is_public_network,
			issue_event_tx.clone(),
			replace_event_tx.clone(),
			vault_public_key.clone(),
			issue_map.clone(),
			ledger_env_map.clone(),
			memos_to_issue_ids.clone(),
		)?;

		let mut issue_tasks = self.create_issue_tasks(
			issue_event_tx.clone(),
			issue_event_rx,
			startup_height,
			account_id.clone(),
			vault_public_key,
			oracle_agent.clone(),
			issue_map,
			ledger_env_map,
			memos_to_issue_ids,
		);

		tasks.append(&mut issue_tasks);

		let mut replace_tasks = self.create_replace_tasks(
			replace_event_tx.clone(),
			replace_event_rx,
			startup_height,
			account_id,
			oracle_agent.clone(),
		);

		tasks.append(&mut replace_tasks);

		tasks.push((
			"Redeem Request Listener",
			run(listen_for_redeem_requests(
				self.shutdown.clone(),
				self.spacewalk_parachain.clone(),
				self.vault_id_manager.clone(),
				self.config.payment_margin_minutes,
				oracle_agent,
			)),
		));

		let mut bridge_metrics_tasks = self.create_bridge_metrics_tasks();

		tasks.append(&mut bridge_metrics_tasks);

		Ok(tasks)
	}

	fn stellar_overlay_cfg(&self) -> Result<StellarOverlayConfig, Error> {
		let cfg_path = &self.config.stellar_overlay_config_filepath;
		StellarOverlayConfig::try_from_path(cfg_path).map_err(Error::StellarRelayError)
	}
}

impl VaultService {
	async fn new(
		spacewalk_parachain: SpacewalkParachain,
		config: VaultServiceConfig,
		monitoring_config: MonitoringConfig,
		shutdown: ShutdownSender,
	) -> Result<Self, Error> {
		let is_public_network = spacewalk_parachain.is_public_network().await;

		let secret_key = fs::read_to_string(&config.stellar_vault_secret_key_filepath)?
			.trim()
			.to_string();
		let stellar_wallet = StellarWallet::from_secret_encoded(&secret_key, is_public_network)?;
		tracing::debug!(
			"Vault wallet public key: {}",
			from_utf8(&stellar_wallet.public_key().to_encoding())?
		);

		let stellar_wallet = Arc::new(RwLock::new(stellar_wallet));

		Ok(Self {
			spacewalk_parachain: spacewalk_parachain.clone(),
			stellar_wallet: stellar_wallet.clone(),
			config,
			monitoring_config,
			shutdown,
			vault_id_manager: VaultIdManager::new(spacewalk_parachain, stellar_wallet),
			secret_key,
			agent: None,
		})
	}

	fn secret_key(&self) -> String {
		self.secret_key.clone()
	}

	fn get_vault_id(
		&self,
		collateral_currency: CurrencyId,
		wrapped_currency: CurrencyId,
	) -> VaultId {
		VaultId {
			account_id: self.spacewalk_parachain.get_account_id().clone(),
			currencies: VaultCurrencyPair {
				collateral: subxt::utils::Static(collateral_currency),
				wrapped: subxt::utils::Static(wrapped_currency),
			},
		}
	}

	async fn run_service(&mut self) -> Result<(), ServiceError<Error>> {
		let startup_height = self.await_parachain_block().await?;
		let account_id = self.spacewalk_parachain.get_account_id().clone();

		tracing::info!("Starting client service...");

		let parsed_auto_register = self.auto_register()?;

		self.maintain_connection()?;

		self.register_public_key_if_not_present().await?;

		join_all(parsed_auto_register.iter().map(
			|(collateral_currency, wrapped_currency, amount)| {
				self.register_vault_if_not_present(collateral_currency, wrapped_currency, amount)
			},
		))
		.await
		.into_iter()
		.collect::<Result<_, Error>>()?;

		// purposefully _after_ register_vault_if_not_present and _before_ other calls
		self.vault_id_manager.fetch_vault_ids().await?;

		let mut wallet = self.stellar_wallet.write().await;
		let vault_public_key = wallet.public_key();
		let is_public_network = wallet.is_public_network();

		// re-submit transactions in the cache
		wallet
			.start_periodic_resubmission_of_transactions_from_cache(RESUBMISSION_INTERVAL_IN_SECS)
			.await;
		drop(wallet);

		let oracle_agent =
			self.create_oracle_agent(is_public_network, self.shutdown.clone()).await?;
		self.agent = Some(oracle_agent.clone());

		// issue handling
		// this vec is passed to the stellar wallet to filter out transactions that are not relevant
		// this has to be modified every time the issue set changes
		let issue_map: ArcRwLock<IssueRequestsMap> = Arc::new(RwLock::new(IssueRequestsMap::new()));
		// this map resolves issue memo to issue ids
		let memos_to_issue_ids: ArcRwLock<IssueIdLookup> =
			Arc::new(RwLock::new(IssueIdLookup::new()));

		issue::initialize_issue_set(&self.spacewalk_parachain, &issue_map, &memos_to_issue_ids)
			.await?;

		let (precheck_sender, precheck_receivers) = precheck_signals(8);
		tokio_spawn(
			"Execute Open Requests",
			execute_open_requests(
				self.shutdown.clone(),
				self.spacewalk_parachain.clone(),
				self.vault_id_manager.clone(),
				self.stellar_wallet.clone(),
				oracle_agent.clone(),
				self.config.payment_margin_minutes,
				precheck_sender,
			),
		);

		let ledger_env_map: ArcRwLock<LedgerTxEnvMap> = Arc::new(RwLock::new(HashMap::new()));

		tracing::info!("Starting all services...");

		let mut tasks = vec![(
			"Stellar Messages Listener",
			run(listen_for_stellar_messages(oracle_agent.clone(), self.shutdown.clone())),
		)];

		let mut _tasks = self.create_tasks(
			startup_height,
			account_id,
			is_public_network,
			vault_public_key,
			oracle_agent,
			issue_map,
			ledger_env_map,
			memos_to_issue_ids,
		)?;
		tasks.append(&mut _tasks);

		run_and_monitor_tasks(self.shutdown.clone(), tasks, precheck_receivers).await
	}

	async fn register_public_key_if_not_present(&mut self) -> Result<(), Error> {
		let _ = self.try_fund_from_faucet().await;

		if self.spacewalk_parachain.get_public_key().await?.is_none() {
			let public_key = self.stellar_wallet.read().await.public_key();
			let pub_key_encoded = public_key.to_encoding();
			tracing::info!(
				"Registering public key to the parachain...{}",
				from_utf8(&pub_key_encoded)?
			);
			self.spacewalk_parachain.register_public_key(public_key.into_binary()).await?;
		} else {
			tracing::info!("Not registering public key -- already registered");
		}

		Ok(())
	}

	/// Only works when the stellar network is testnet
	async fn try_fund_from_faucet(&self) -> bool {
		let Some(faucet_url) = &self.config.faucet_url else { return false };

		let is_public_network = self.spacewalk_parachain.is_public_network().await;

		// fund the account if on stellar TESTNET
		if !is_public_network {
			let account_id = self.spacewalk_parachain.get_account_id().pretty_print();
			let url = format!("{faucet_url}?to={account_id}");
			match reqwest::get(url.clone()).await {
				Ok(response) if response.status().is_success() => {
					tracing::info!("try_fund_from_faucet(): successful funded {account_id}");
					return true;
				},
				Ok(response) => {
					tracing::error!("try_fund_from_faucet(): failed to fund {account_id} from faucet: {response:#?}");
				},
				Err(e) => {
					tracing::error!(
						"try_fund_from_faucet(): failed to fund {account_id} from faucet: {e}"
					);
				},
			}
		}

		false
	}

	async fn register_vault_with_collateral(
		&self,
		vault_id: VaultId,
		collateral_amount: &Option<u128>,
	) -> Result<(), Error> {
		if let Some(collateral) = collateral_amount {
			tracing::info!("[{}] Automatically registering...", vault_id.pretty_print());
			let free_balance = self
				.spacewalk_parachain
				.get_free_balance(vault_id.collateral_currency())
				.await?;
			return self
				.spacewalk_parachain
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
				.await
				.map_err(|e| Error::RuntimeError(e));
		} else if self.try_fund_from_faucet().await {
			Ok(())
		} else {
			tracing::error!(
				"[{}] Cannot register a vault: no collateral and no faucet url",
				vault_id.pretty_print()
			);
			Err(Error::FaucetUrlNotSet)
		}
	}

	async fn register_vault_if_not_present(
		&self,
		collateral_currency: &CurrencyId,
		wrapped_currency: &CurrencyId,
		maybe_collateral_amount: &Option<u128>,
	) -> Result<(), Error> {
		let vault_id = self.get_vault_id(*collateral_currency, *wrapped_currency);

		// check if a vault is registered
		match self.spacewalk_parachain.get_vault(&vault_id).await {
			Ok(_) | Err(RuntimeError::VaultLiquidated) => {
				tracing::info!(
					"[{}] Not registering vault -- already registered",
					vault_id.pretty_print()
				);
				Ok(())
			},
			Err(RuntimeError::VaultNotFound) => {
				tracing::info!("[{}] Not registered", vault_id.pretty_print());
				self.register_vault_with_collateral(vault_id, maybe_collateral_amount).await
			},
			Err(e) => Err(Error::RuntimeError(e)),
		}
	}

	async fn await_parachain_block(&self) -> Result<u32, Error> {
		// wait for a new block to arrive, to prevent processing an event that potentially
		// has been processed already prior to restarting
		let startup_height = self.spacewalk_parachain.get_current_chain_height().await?;
		while startup_height == self.spacewalk_parachain.get_current_chain_height().await? {
			sleep(CHAIN_HEIGHT_POLLING_INTERVAL).await;
		}
		tracing::info!("Got new block at height {startup_height}");
		Ok(startup_height)
	}

	/// shuts down the resubmission task running in the background
	async fn try_shutdown_wallet(&self) {
		tracing::info!("try_shutdown_wallet(): stop the resubmission scheduler");
		let mut wallet = self.stellar_wallet.write().await;
		wallet.try_stop_periodic_resubmission_of_transactions().await;
		drop(wallet);
	}
}
