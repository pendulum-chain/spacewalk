use std::{fmt, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::{future::Either, Future, FutureExt};
use governor::{Quota, RateLimiter};
use nonzero_ext::*;
use tokio::sync::RwLock;
pub use warp;

pub use cli::{LoggingFormat, RestartPolicy, ServiceConfig};
pub use error::Error;
use runtime::{
	cli::ConnectionOpts as ParachainConfig, CurrencyId, PrettyPrint, ShutdownReceiver,
	ShutdownSender, SpacewalkParachain, SpacewalkSigner,
};
pub use trace::init_subscriber;

mod cli;
mod error;
mod trace;

#[async_trait]
pub trait Service<Config, InnerError> {
	const NAME: &'static str;
	const VERSION: &'static str;

	fn new_service(
		spacewalk_parachain: SpacewalkParachain,
		config: Config,
		shutdown: ShutdownSender,
	) -> Result<Self, InnerError>
	where
		Self: Sized;
	async fn start(&mut self) -> Result<(), Error<InnerError>>;
}

pub struct ConnectionManager<Config: Clone, F: Fn()> {
	signer: Arc<RwLock<SpacewalkSigner>>,
	wallet_name: Option<String>,
	parachain_config: ParachainConfig,
	service_config: ServiceConfig,
	config: Config,
	increment_restart_counter: F,
}

impl<Config: Clone + Send + 'static, F: Fn()> ConnectionManager<Config, F> {
	#[allow(clippy::too_many_arguments)]
	pub fn new(
		signer: Arc<RwLock<SpacewalkSigner>>,
		wallet_name: Option<String>,
		parachain_config: ParachainConfig,
		service_config: ServiceConfig,
		config: Config,
		increment_restart_counter: F,
	) -> Self {
		Self {
			signer,
			wallet_name,
			parachain_config,
			service_config,
			config,
			increment_restart_counter,
		}
	}

	pub async fn start<S: Service<Config, InnerError>, InnerError: fmt::Display>(
		&self,
	) -> Result<(), Error<InnerError>> {
		loop {
			tracing::info!("Version: {}", S::VERSION);
			tracing::info!("AccountId: {}", self.signer.read().await.account_id().pretty_print());

			let config = self.config.clone();
			let shutdown_tx = ShutdownSender::new();

			let signer = self.signer.clone();
			let spacewalk_parachain = SpacewalkParachain::from_url_and_config_with_retry(
				&self.parachain_config.spacewalk_parachain_url,
				signer,
				self.parachain_config.max_concurrent_requests,
				self.parachain_config.max_notifs_per_subscription,
				self.parachain_config.spacewalk_parachain_connection_timeout_ms,
				shutdown_tx.clone(),
			)
			.await?;

			let mut service = S::new_service(spacewalk_parachain, config, shutdown_tx.clone())
				.map_err(|e| Error::StartServiceError(e))?;

			match service.start().await {
				Err(err @ Error::Abort(_)) => {
					tracing::warn!("Disconnected: {}", err);
					return Err(err)
				},
				Err(err) => {
					tracing::warn!("Disconnected: {}", err);
				},
				_ => {
					tracing::warn!("Disconnected");
				},
			}

			let rate_limiter = RateLimiter::direct(Quota::per_minute(nonzero!(4u32)));

			loop {
				match shutdown_tx.receiver_count() {
					0 => break,
					count => {
						if rate_limiter.check().is_ok() {
							tracing::error!("Waiting for {count} tasks to shut down...");
						}
						tokio::time::sleep(Duration::from_secs(1)).await;
					},
				}
			}
			tracing::info!("All tasks successfully shut down");

			match self.service_config.restart_policy {
				RestartPolicy::Never => return Err(Error::ClientShutdown),
				RestartPolicy::Always => {
					(self.increment_restart_counter)();
					continue
				},
			};
		}
	}
}

pub async fn wait_or_shutdown<F, E>(shutdown_tx: ShutdownSender, future2: F) -> Result<(), E>
where
	F: Future<Output = Result<(), E>>,
{
	match run_cancelable(shutdown_tx.subscribe(), future2).await {
		TerminationStatus::Cancelled => {
			tracing::trace!("Received shutdown signal");
			Ok(())
		},
		TerminationStatus::Completed(res) => {
			tracing::trace!("Sending shutdown signal");
			let _ = shutdown_tx.send(());
			res
		},
	}
}

pub enum TerminationStatus<Res> {
	Cancelled,
	Completed(Res),
}

async fn run_cancelable<F, Res>(
	mut shutdown_rx: ShutdownReceiver,
	future2: F,
) -> TerminationStatus<Res>
where
	F: Future<Output = Res>,
{
	let future1 = shutdown_rx.recv().fuse();
	let future2 = future2.fuse();

	futures::pin_mut!(future1);
	futures::pin_mut!(future2);

	match futures::future::select(future1, future2).await {
		Either::Left((_, _)) => TerminationStatus::Cancelled,
		Either::Right((res, _)) => TerminationStatus::Completed(res),
	}
}

pub fn spawn_cancelable<T: Future + Send + 'static>(shutdown_rx: ShutdownReceiver, future: T)
where
	<T as futures::Future>::Output: Send,
{
	tokio::spawn(run_cancelable(shutdown_rx, future));
}

pub async fn on_shutdown(shutdown_tx: ShutdownSender, future2: impl Future) {
	let mut shutdown_rx = shutdown_tx.subscribe();
	let future1 = shutdown_rx.recv().fuse();

	let _ = future1.await;
	future2.await;
}
