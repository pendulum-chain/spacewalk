use crate::{
    deposit::poll_horizon_for_new_transactions, error::Error, redeem::listen_for_redeem_requests,
    CHAIN_HEIGHT_POLLING_INTERVAL,
};
use async_trait::async_trait;
use clap::Parser;
use git_version::git_version;
use runtime::{SpacewalkParachain, UtilFuncs};
use service::{wait_or_shutdown, Error as ServiceError, Service, ShutdownSender};
use tokio::time::sleep;

pub const VERSION: &str = git_version!(args = ["--tags"]);
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Parser, Clone, Debug)]
pub struct VaultServiceConfig {
    #[clap(long, help = "The Stellar secret key that is used to sign transactions.")]
    pub stellar_escrow_secret_key: String,
}

pub struct VaultService {
    spacewalk_parachain: SpacewalkParachain,
    config: VaultServiceConfig,
    shutdown: ShutdownSender,
}

#[async_trait]
impl Service<VaultServiceConfig> for VaultService {
    const NAME: &'static str = NAME;
    const VERSION: &'static str = VERSION;

    fn new_service(
        spacewalk_parachain: SpacewalkParachain,
        config: VaultServiceConfig,
        shutdown: ShutdownSender,
    ) -> Self {
        VaultService::new(spacewalk_parachain, config, shutdown)
    }

    async fn start(&self) -> Result<(), ServiceError> {
        match self.run_service().await {
            Ok(_) => Ok(()),
            Err(Error::RuntimeError(err)) => Err(ServiceError::RuntimeError(err)),
            Err(err) => Err(ServiceError::Other(err.to_string())),
        }
    }
}

impl VaultService {
    fn new(spacewalk_parachain: SpacewalkParachain, config: VaultServiceConfig, shutdown: ShutdownSender) -> Self {
        Self {
            spacewalk_parachain: spacewalk_parachain.clone(),
            config,
            shutdown,
        }
    }

    async fn run_service(&self) -> Result<(), Error> {
        self.await_parachain_block().await?;

        let err_provider = self.spacewalk_parachain.clone();
        let err_listener = wait_or_shutdown(self.shutdown.clone(), async move {
            err_provider
                .on_event_error(|e| tracing::debug!("Received error event: {}", e))
                .await?;
            Ok(())
        });

        let deposit_listener = wait_or_shutdown(
            self.shutdown.clone(),
            poll_horizon_for_new_transactions(
                self.spacewalk_parachain.clone(),
                self.config.stellar_escrow_secret_key.clone(),
            ),
        );

        // redeem handling
        let redeem_listener = wait_or_shutdown(
            self.shutdown.clone(),
            listen_for_redeem_requests(
                self.shutdown.clone(),
                self.spacewalk_parachain.clone(),
                self.config.stellar_escrow_secret_key.clone(),
            ),
        );

        // starts all the tasks
        tracing::info!("Starting to listen for events...");
        let _ = tokio::join!(
            // runs error listener to log errors
            tokio::spawn(async move { err_listener.await }),
            // listen for deposits
            tokio::task::spawn(async move { deposit_listener.await }),
            // listen for redeem events
            tokio::spawn(async move { redeem_listener.await }),
        );

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
