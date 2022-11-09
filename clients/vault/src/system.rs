use crate::{
	//deposit::poll_horizon_for_new_transactions,
	error::Error,
	//redeem::listen_for_redeem_requests,
	CHAIN_HEIGHT_POLLING_INTERVAL,
};
use crate::{
	issue::{
		listen_for_cancel_requests, listen_for_execute_requests, listen_for_issue_requests,
		process_issue_events, IssueActions,
	},
	oracle::{create_handler, prepare_directories, ScpMessageHandler},
};
use async_trait::async_trait;
use clap::Parser;
use git_version::git_version;
use runtime::{IssuePallet, IssueRequests, RequestIssueEvent, SpacewalkParachain, UtilFuncs};
use service::{wait_or_shutdown, Error as ServiceError, Service, ShutdownSender};
use std::sync::Arc;
use stellar_relay_lib::{
	node::NodeInfo,
	sdk::{
		network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
		SecretKey,
	},
	ConnConfig,
};
use tokio::{
	sync::{mpsc, Mutex},
	time::sleep,
};

pub const VERSION: &str = git_version!(args = ["--tags"], fallback = "unknown");
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

// sdftest3
pub const TIER_1_VALIDATOR_IP_TESTNET: &str = "3.239.7.78";
// SatoshiPay (DE, Frankfurt)
pub const TIER_1_VALIDATOR_IP_PUBLIC: &str = "141.95.47.112";

#[derive(Parser, Clone, Debug)]
pub struct VaultServiceConfig {
	#[clap(long, help = "The Stellar secret key that is used to sign transactions.")]
	pub stellar_vault_secret_key: String,
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
	fn new(
		spacewalk_parachain: SpacewalkParachain,
		config: VaultServiceConfig,
		shutdown: ShutdownSender,
	) -> Self {
		Self { spacewalk_parachain: spacewalk_parachain.clone(), config, shutdown }
	}

	/// Returns SCPMessageHandler which contains the thread to connect/listen to the Stellar
	/// Node. See the oracle.rs example
	async fn create_handler(&self) -> Result<ScpMessageHandler, Error> {
		prepare_directories().map_err(|e| {
			tracing::error!("Failed to create the SCPMessageHandler: {:?}", e);
			Error::StellarSdkError
		})?;

		let is_public_net =
			self.spacewalk_parachain.is_public_network().await.map_err(Error::from)?;

		let tier1_node_ip =
			if is_public_net { TIER_1_VALIDATOR_IP_PUBLIC } else { TIER_1_VALIDATOR_IP_TESTNET };

		let network: &Network = if is_public_net { &PUBLIC_NETWORK } else { &TEST_NETWORK };

		tracing::info!(
			"Connecting to {:?} through {:?}",
			std::str::from_utf8(network.get_passphrase().as_slice()).unwrap(),
			tier1_node_ip
		);

		let secret = SecretKey::from_encoding(&self.config.stellar_vault_secret_key).unwrap();
		let public_key_binary = hex::encode(secret.get_public().as_binary());
		tracing::info!("public key binary {:?}", public_key_binary);

		let node_info = NodeInfo::new(19, 21, 19, "v19.1.0".to_string(), network);
		let cfg = ConnConfig::new(tier1_node_ip, 11625, secret, 0, true, true, false);

		// todo: add vault addresses filter

		let addresses = vec![];
		create_handler(node_info, cfg, is_public_net, addresses).await.map_err(|e| {
			tracing::error!("Failed to create the SCPMessageHandler: {:?}", e);
			Error::StellarSdkError
		})
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

		let (issue_events_tx, issue_events_rx) = mpsc::channel::<IssueActions>(32);

		let issue_requests_listener = wait_or_shutdown(
			self.shutdown.clone(),
			listen_for_issue_requests(
				self.spacewalk_parachain.clone(),
				self.config.stellar_vault_secret_key.clone(),
				issue_events_tx.clone(),
			),
		);

		let issue_cancels_listener = wait_or_shutdown(
			self.shutdown.clone(),
			listen_for_cancel_requests(self.spacewalk_parachain.clone(), issue_events_tx.clone()),
		);

		let issue_executes_listener = wait_or_shutdown(
			self.shutdown.clone(),
			listen_for_execute_requests(self.spacewalk_parachain.clone(), issue_events_tx.clone()),
		);

		// issue handling
		let issue_set: Arc<Mutex<IssueRequests>> = Arc::new(Mutex::new(IssueRequests::new()));

		// todo: get old issue requests, but it needs the vault id?
		// self.spacewalk_parachain.get_vault_issue_requests()

		let handler = {
			let handler = self.create_handler().await?;
			Arc::new(Mutex::new(handler))
		};

		let issue_handling_task = wait_or_shutdown(
			self.shutdown.clone(),
			process_issue_events(
				self.spacewalk_parachain.clone(),
				handler.clone(),
				issue_events_rx,
				issue_set.clone(),
			),
		);

		// starts all the tasks
		tracing::info!("Starting to listen for events...");
		let _ = tokio::join!(
			// runs error listener to log errors
			tokio::spawn(async move { err_listener.await }),
			// listen for issue requests
			tokio::spawn(async move { issue_requests_listener.await }),
			// listen for cancelled issue requests
			tokio::spawn(async move { issue_cancels_listener.await }),
			// listen for executed issue requests
			tokio::spawn(async move { issue_executes_listener.await }),
			// runs the handler
			tokio::spawn(async move { issue_handling_task.await })
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
