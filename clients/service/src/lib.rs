use async_trait::async_trait;
use futures::{future::Either, Future, FutureExt};
use runtime::{
    cli::ConnectionOpts as ParachainConfig, Error as RuntimeError,
    InterBtcParachain as BtcParachain, InterBtcSigner, Signer, Ss58Codec,
};
use std::marker::PhantomData;

mod cli;
mod error;
mod trace;

pub use cli::{LoggingFormat, RestartPolicy, ServiceConfig};
pub use error::Error;
pub use trace::init_subscriber;

pub type ShutdownSender = tokio::sync::broadcast::Sender<Option<()>>;
pub type ShutdownReceiver = tokio::sync::broadcast::Receiver<Option<()>>;

#[async_trait]
pub trait Service<Config> {
    const NAME: &'static str;
    const VERSION: &'static str;

    fn new_service(btc_parachain: BtcParachain, config: Config, shutdown: ShutdownSender) -> Self;
    async fn start(&self) -> Result<(), Error>;
}

pub struct ConnectionManager<Config: Clone, S: Service<Config>> {
    signer: InterBtcSigner,
    wallet_name: Option<String>,
    parachain_config: ParachainConfig,
    service_config: ServiceConfig,
    config: Config,
    _marker: PhantomData<S>,
}

impl<Config: Clone + Send + 'static, S: Service<Config>> ConnectionManager<Config, S> {
    pub fn new(
        signer: InterBtcSigner,
        wallet_name: Option<String>,
        parachain_config: ParachainConfig,
        service_config: ServiceConfig,
        config: Config,
    ) -> Self {
        Self {
            signer,
            wallet_name,
            parachain_config,
            service_config,
            config,
            _marker: PhantomData::default(),
        }
    }
}

impl<Config: Clone + Send + 'static, S: Service<Config>> ConnectionManager<Config, S> {
    pub async fn start(&self) -> Result<(), Error> {
        loop {
            tracing::info!("Version: {}", S::VERSION);
            tracing::info!("AccountId: {}", self.signer.account_id().to_ss58check());

            let config = self.config.clone();
            let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);

            // only open connection to parachain after bitcoind sync to prevent timeout
            let signer = self.signer.clone();
            let btc_parachain = BtcParachain::from_url_and_config_with_retry(
                &self.parachain_config.btc_parachain_url,
                signer,
                self.parachain_config.max_concurrent_requests,
                self.parachain_config.max_notifs_per_subscription,
                self.parachain_config.btc_parachain_connection_timeout_ms,
            )
            .await?;

            let service = S::new_service(btc_parachain, config, shutdown_tx);
            if let Err(outer) = service.start().await {
                match outer {
                    Error::RuntimeError(RuntimeError::ChannelClosed) => (),
                    Error::RuntimeError(ref inner) if inner.is_rpc_error() => (),
                    other => return Err(other),
                };
                tracing::info!("Disconnected: {}", outer);
            } else {
                tracing::info!("Disconnected");
            }

            match self.service_config.restart_policy {
                RestartPolicy::Never => return Err(Error::ClientShutdown),
                RestartPolicy::Always => continue,
            };
        }
    }
}

pub async fn wait_or_shutdown<F>(shutdown_tx: ShutdownSender, future2: F)
where
    F: Future<Output = Result<(), Error>>,
{
    match run_cancelable(shutdown_tx.subscribe(), future2).await {
        TerminationStatus::Cancelled => {
            tracing::trace!("Received shutdown signal");
        }
        TerminationStatus::Completed => {
            tracing::trace!("Sending shutdown signal");
            // TODO: shutdown signal should be error
            let _ = shutdown_tx.send(Some(()));
        }
    }
}

pub enum TerminationStatus {
    Cancelled,
    Completed,
}
async fn run_cancelable<F, Ret>(mut shutdown_rx: ShutdownReceiver, future2: F) -> TerminationStatus
where
    F: Future<Output = Ret>,
{
    let future1 = shutdown_rx.recv().fuse();
    let future2 = future2.fuse();

    futures::pin_mut!(future1);
    futures::pin_mut!(future2);

    match futures::future::select(future1, future2).await {
        Either::Left((_, _)) => TerminationStatus::Cancelled,
        Either::Right((_, _)) => TerminationStatus::Completed,
    }
}

pub fn spawn_cancelable<T: Future + Send + 'static>(shutdown_rx: ShutdownReceiver, future: T) {
    tokio::spawn(run_cancelable(shutdown_rx, future));
}

pub async fn on_shutdown(shutdown_tx: ShutdownSender, future2: impl Future) {
    let mut shutdown_rx = shutdown_tx.subscribe();
    let future1 = shutdown_rx.recv().fuse();

    let _ = future1.await;
    future2.await;
}
