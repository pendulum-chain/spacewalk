use crate::{
    collateral::lock_required_collateral, error::Error, faucet,
    horizon::fetch_horizon_txs_and_process_new_transactions, issue, relay::run_relayer, service::*, vaults::Vaults,
    Event, IssueRequests, CHAIN_HEIGHT_POLLING_INTERVAL,
};
use async_trait::async_trait;
use bitcoin::{BitcoinCore, BitcoinCoreApi, Error as BitcoinError};
use clap::Parser;
use futures::{
    channel::{mpsc, mpsc::Sender},
    executor::block_on,
    Future, SinkExt,
};
use git_version::git_version;
use runtime::{
    cli::{parse_duration_minutes, parse_duration_ms},
    parse_collateral_currency, BtcRelayPallet, CollateralBalancesPallet, CurrencyId, Error as RuntimeError,
    InterBtcParachain, RegisterVaultEvent, StoreMainChainHeaderEvent, UpdateActiveBlockEvent, UtilFuncs,
    VaultCurrencyPair, VaultId, VaultRegistryPallet,
};
use service::{wait_or_shutdown, Error as ServiceError, Service, ShutdownSender};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{sync::RwLock, time::sleep};

pub const VERSION: &str = git_version!(args = ["--tags"]);
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Parser, Clone, Debug)]
pub struct VaultServiceConfig {
    /// Automatically register the vault with the given amount of collateral and a newly generated address.
    #[clap(long)]
    pub auto_register_with_collateral: Option<u128>,

    /// Automatically register the vault with the collateral received from the faucet and a newly generated address.
    /// The parameter is the URL of the faucet
    #[clap(long, conflicts_with("auto-register-with-collateral"))]
    pub auto_register_with_faucet_url: Option<String>,

    /// Opt out of participation in replace requests.
    #[clap(long)]
    pub no_auto_replace: bool,

    /// Don't try to execute issues.
    #[clap(long)]
    pub no_issue_execution: bool,

    /// Don't run the RPC API.
    #[clap(long)]
    pub no_api: bool,

    /// Timeout in milliseconds to repeat collateralization checks.
    #[clap(long, parse(try_from_str = parse_duration_ms), default_value = "5000")]
    pub collateral_timeout_ms: Duration,

    /// How many bitcoin confirmations to wait for. If not specified, the
    /// parachain settings will be used (recommended).
    #[clap(long)]
    pub btc_confirmations: Option<u32>,

    /// Minimum time to the the redeem/replace execution deadline to make the bitcoin payment.
    #[clap(long, parse(try_from_str = parse_duration_minutes), default_value = "120")]
    pub payment_margin_minutes: Duration,

    /// Starting height for vault theft checks, if not defined
    /// automatically start from the chain tip.
    #[clap(long)]
    pub bitcoin_theft_start_height: Option<u32>,

    /// Timeout in milliseconds to poll Bitcoin.
    #[clap(long, parse(try_from_str = parse_duration_ms), default_value = "6000")]
    pub bitcoin_poll_interval_ms: Duration,

    /// Starting height to relay block headers, if not defined
    /// use the best height as reported by the relay module.
    #[clap(long)]
    pub bitcoin_relay_start_height: Option<u32>,

    /// Max batch size for combined block header submission.
    #[clap(long, default_value = "16")]
    pub max_batch_size: u32,

    /// Number of confirmations a block needs to have before it is submitted.
    #[clap(long, default_value = "0")]
    pub bitcoin_relay_confirmations: u32,

    /// Don't relay bitcoin block headers.
    #[clap(long)]
    pub no_bitcoin_block_relay: bool,

    /// Don't monitor vault thefts.
    #[clap(long)]
    pub no_vault_theft_report: bool,

    /// Don't refund overpayments.
    #[clap(long)]
    pub no_auto_refund: bool,

    /// The currency to use for the collateral, e.g. "DOT" or "KSM".
    /// Defaults to the relay chain currency if not set.
    #[clap(long, parse(try_from_str = parse_collateral_currency))]
    pub collateral_currency_id: Option<CurrencyId>,
}

async fn active_block_listener(
    parachain_rpc: InterBtcParachain,
    issue_tx: Sender<Event>,
    replace_tx: Sender<Event>,
) -> Result<(), ServiceError> {
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

async fn relay_block_listener(
    parachain_rpc: InterBtcParachain,
    issue_tx: Sender<Event>,
    replace_tx: Sender<Event>,
) -> Result<(), ServiceError> {
    let issue_tx = &issue_tx;
    let replace_tx = &replace_tx;
    parachain_rpc
        .on_event::<StoreMainChainHeaderEvent, _, _, _>(
            |event| async move {
                let _ = issue_tx.clone().send(Event::BitcoinBlock(event.block_height)).await;
                let _ = replace_tx.clone().send(Event::BitcoinBlock(event.block_height)).await;
            },
            |err| tracing::error!("Error (StoreMainChainHeaderEvent): {}", err.to_string()),
        )
        .await?;
    Ok(())
}

#[derive(Clone)]
pub struct VaultIdManager<BCA: BitcoinCoreApi + Clone + Send + Sync + 'static> {
    bitcoin_rpcs: Arc<RwLock<HashMap<VaultId, BCA>>>,
    btc_parachain: InterBtcParachain,
    // TODO: refactor this
    #[allow(clippy::type_complexity)]
    constructor: Arc<Box<dyn Fn(VaultId) -> Result<BCA, BitcoinError> + Send + Sync>>,
}

impl<BCA: BitcoinCoreApi + Clone + Send + Sync + 'static> VaultIdManager<BCA> {
    pub fn new(
        btc_parachain: InterBtcParachain,
        constructor: impl Fn(VaultId) -> Result<BCA, BitcoinError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            bitcoin_rpcs: Arc::new(RwLock::new(HashMap::new())),
            constructor: Arc::new(Box::new(constructor)),
            btc_parachain,
        }
    }

    // used for testing only
    pub fn from_map(btc_parachain: InterBtcParachain, map: HashMap<VaultId, BCA>) -> Self {
        Self {
            bitcoin_rpcs: Arc::new(RwLock::new(map)),
            constructor: Arc::new(Box::new(|_| unimplemented!())),
            btc_parachain,
        }
    }

    async fn add_vault_id(&self, vault_id: VaultId) -> Result<BCA, Error> {
        let btc_rpc = (*self.constructor)(vault_id.clone())?;

        // load wallet. Exit on failure, since without wallet we can't do a lot
        btc_rpc
            .create_or_load_wallet()
            .await
            .map_err(Error::WalletInitializationFailure)?;

        if let Ok(vault) = self.btc_parachain.get_vault(&vault_id).await {
            if !btc_rpc.wallet_has_public_key(vault.wallet.public_key.0).await? {
                return Err(bitcoin::Error::MissingPublicKey.into());
            }
        }
        issue::add_keys_from_past_issue_request(&btc_rpc, &self.btc_parachain).await?;

        self.bitcoin_rpcs.write().await.insert(vault_id, btc_rpc.clone());

        Ok(btc_rpc)
    }

    pub async fn fetch_vault_ids(&self, startup_collateral_increase: bool) -> Result<(), Error> {
        for vault_id in self
            .btc_parachain
            .get_vaults_by_account_id(self.btc_parachain.get_account_id())
            .await?
        {
            self.add_vault_id(vault_id.clone()).await?;

            if startup_collateral_increase {
                // check if the vault is registered
                match lock_required_collateral(self.btc_parachain.clone(), vault_id).await {
                    Err(Error::RuntimeError(runtime::Error::VaultNotFound)) => {} // not registered
                    Err(e) => tracing::error!("Failed to lock required additional collateral: {}", e),
                    _ => {} // collateral level now OK
                };
            }
        }
        Ok(())
    }

    pub async fn listen_for_vault_id_registrations(self) -> Result<(), ServiceError> {
        Ok(self
            .btc_parachain
            .on_event::<RegisterVaultEvent, _, _, _>(
                |event| async {
                    let vault_id = event.vault_id;
                    if self.btc_parachain.is_this_vault(&vault_id) {
                        tracing::info!("New vault registered: {}", vault_id.pretty_printed());
                        let _ = self.add_vault_id(vault_id).await;
                    }
                },
                |err| tracing::error!("Error (RegisterVaultEvent): {}", err.to_string()),
            )
            .await?)
    }

    pub async fn get_bitcoin_rpc(&self, vault_id: &VaultId) -> Option<BCA> {
        self.bitcoin_rpcs.read().await.get(vault_id).cloned()
    }

    pub async fn get_vault_ids(&self) -> Vec<VaultId> {
        self.bitcoin_rpcs
            .read()
            .await
            .iter()
            .map(|(vault_id, _)| vault_id.clone())
            .collect()
    }

    pub async fn get_vault_btc_rpcs(&self) -> Vec<(VaultId, BCA)> {
        self.bitcoin_rpcs
            .read()
            .await
            .iter()
            .map(|(vault_id, btc_rpc)| (vault_id.clone(), btc_rpc.clone()))
            .collect()
    }
}

pub struct VaultService {
    btc_parachain: InterBtcParachain,
    config: VaultServiceConfig,
    shutdown: ShutdownSender,
}

#[async_trait]
impl Service<VaultServiceConfig> for VaultService {
    const NAME: &'static str = NAME;
    const VERSION: &'static str = VERSION;

    fn new_service(btc_parachain: InterBtcParachain, config: VaultServiceConfig, shutdown: ShutdownSender) -> Self {
        VaultService::new(btc_parachain, config, shutdown)
    }

    async fn start(&self) -> Result<(), ServiceError> {
        match self.run_service().await {
            Ok(_) => Ok(()),
            Err(Error::RuntimeError(err)) => Err(ServiceError::RuntimeError(err)),
            Err(Error::BitcoinError(err)) => Err(ServiceError::BitcoinError(err)),
            Err(err) => Err(ServiceError::Other(err.to_string())),
        }
    }
}

async fn maybe_run_task(should_run: bool, task: impl Future) {
    if should_run {
        task.await;
    }
}

impl VaultService {
    fn new(btc_parachain: InterBtcParachain, config: VaultServiceConfig, shutdown: ShutdownSender) -> Self {
        Self {
            btc_parachain: btc_parachain.clone(),
            config,
            shutdown,
        }
    }

    async fn run_service(&self) -> Result<(), Error> {
        let account_id = self.btc_parachain.get_account_id().clone();

        let num_confirmations = match self.config.btc_confirmations {
            Some(x) => x,
            None => self.btc_parachain.get_bitcoin_confirmations().await?,
        };
        tracing::info!("Using {} bitcoin confirmations", num_confirmations);

        self.maybe_register_vault().await?;

        // purposefully _after_ maybe_register_vault
        // self.vault_id_manager.fetch_vault_ids(false).await?;

        let startup_height = self.await_parachain_block().await?;

        let open_request_executor = execute_open_requests(
            self.shutdown.clone(),
            self.btc_parachain.clone(),
            num_confirmations,
            self.config.payment_margin_minutes,
            !self.config.no_auto_refund,
        );
        tokio::spawn(async move {
            tracing::info!("Checking for open requests...");
            match open_request_executor.await {
                Ok(_) => tracing::info!("Done processing open requests"),
                Err(e) => tracing::error!("Failed to process open requests: {}", e),
            }
        });

        // get the relay chain tip but don't error because the relay may not be initialized
        let initial_btc_height = self.btc_parachain.get_best_block_height().await.unwrap_or_default();

        // issue handling
        let (issue_event_tx, issue_event_rx) = mpsc::channel::<Event>(32);
        // replace handling
        let (replace_event_tx, replace_event_rx) = mpsc::channel::<Event>(16);

        // listen for parachain blocks, used for cancellation
        let parachain_block_listener = wait_or_shutdown(
            self.shutdown.clone(),
            active_block_listener(
                self.btc_parachain.clone(),
                issue_event_tx.clone(),
                replace_event_tx.clone(),
            ),
        );

        let err_provider = self.btc_parachain.clone();
        let err_listener = wait_or_shutdown(self.shutdown.clone(), async move {
            err_provider
                .on_event_error(|e| tracing::debug!("Received error event: {}", e))
                .await?;
            Ok(())
        });

        // Start polling horizon every 5 seconds
        let mut interval_timer = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval_timer.tick().await;
            tokio::spawn(async {
                fetch_horizon_txs_and_process_new_transactions().await;
            });
        }

        // starts all the tasks
        tracing::info!("Starting to listen for events...");
        let _ = tokio::join!(
            // runs error listener to log errors
            tokio::spawn(async move { err_listener.await }),
            // replace & issue cancellation helpers
            tokio::spawn(async move { parachain_block_listener.await }),
        );


        Ok(())
    }

    async fn maybe_register_vault(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn await_parachain_block(&self) -> Result<u32, Error> {
        // wait for a new block to arrive, to prevent processing an event that potentially
        // has been processed already prior to restarting
        tracing::info!("Waiting for new block...");
        let startup_height = self.btc_parachain.get_current_chain_height().await?;
        while startup_height == self.btc_parachain.get_current_chain_height().await? {
            sleep(CHAIN_HEIGHT_POLLING_INTERVAL).await;
        }
        tracing::info!("Got new block...");
        Ok(startup_height)
    }
}

pub(crate) async fn is_vault_registered(parachain_rpc: &InterBtcParachain, vault_id: &VaultId) -> Result<bool, Error> {
    match parachain_rpc.get_vault(vault_id).await {
        Ok(_) => Ok(true),
        Err(RuntimeError::VaultNotFound) => Ok(false),
        Err(err) => Err(err.into()),
    }
}
