use std::{future::Future, ops::RangeInclusive, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::{stream::StreamExt, FutureExt, SinkExt};
use jsonrpsee::core::{client::Client, JsonValue};
use subxt::{
	client::OnlineClient,
	events::StaticEvent,
	rpc::{RpcClient, RpcClientT},
	tx::{PolkadotExtrinsicParams, Signer, TxEvents, TxProgress},
	Error as BasicError, Metadata,
};
// use subxt_client::OnlineClient;
use tokio::{sync::RwLock, time::timeout};

use crate::{
	conn::{new_websocket_client, new_websocket_client_with_retry},
	metadata,
	metadata::{DispatchError, Event as SpacewalkEvent},
	notify_retry,
	types::*,
	AccountId, Error, RetryPolicy, SpacewalkRuntime, SpacewalkSigner, SubxtError,
};

cfg_if::cfg_if! {
	if #[cfg(feature = "standalone-metadata")] {
		const DEFAULT_SPEC_VERSION: RangeInclusive<u32> = 1..=1;
		const DEFAULT_SPEC_NAME: &str = "spacewalk-standalone";
	} else if #[cfg(feature = "parachain-metadata")] {
		const DEFAULT_SPEC_VERSION: RangeInclusive<u32> = 1..=1;
		const DEFAULT_SPEC_NAME: &str = "pendulum-parachain";
	}
}
const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

// type RuntimeApi = metadata::RuntimeApi<SpacewalkRuntime,
// PolkadotExtrinsicParams<SpacewalkRuntime>>;
pub(crate) type ShutdownSender = tokio::sync::broadcast::Sender<Option<()>>;

#[derive(Clone)]
pub struct SpacewalkParachain {
	signer: Arc<SpacewalkSigner>,
	account_id: AccountId,
	api: OnlineClient<SpacewalkRuntime>,
	shutdown_tx: ShutdownSender,
	metadata: Arc<Metadata>,
}

impl SpacewalkParachain {
	pub async fn new(
		rpc_client: Client,
		signer: Arc<SpacewalkSigner>,
		shutdown_tx: ShutdownSender,
	) -> Result<Self, Error> {
		let account_id = signer.account_id().clone();
		let api = OnlineClient::<SpacewalkRuntime>::from_rpc_client(rpc_client).await?;
		// let api: RuntimeApi = ext_client.clone().to_runtime_api();
		let metadata = Arc::new(api.rpc().metadata().await?);

		let runtime_version = api.rpc().runtime_version(None).await?;
		let default_spec_name = &JsonValue::default();
		let spec_name = runtime_version.other.get("specName").unwrap_or(default_spec_name);
		if spec_name == DEFAULT_SPEC_NAME {
			log::info!("spec_name={}", spec_name);
		} else {
			return Err(Error::ParachainMetadataMismatch(
				DEFAULT_SPEC_NAME.into(),
				spec_name.as_str().unwrap_or_default().into(),
			))
		}

		if DEFAULT_SPEC_VERSION.contains(&runtime_version.spec_version) {
			log::info!("spec_version={}", runtime_version.spec_version);
			log::info!("transaction_version={}", runtime_version.transaction_version);
		} else {
			return Err(Error::InvalidSpecVersion(
				DEFAULT_SPEC_VERSION.start().clone(),
				DEFAULT_SPEC_VERSION.end().clone(),
				runtime_version.spec_version,
			))
		}

		let parachain_rpc = Self { api, shutdown_tx, metadata, signer, account_id };
		Ok(parachain_rpc)
	}

	pub async fn from_url(
		url: &str,
		signer: Arc<SpacewalkSigner>,
		shutdown_tx: ShutdownSender,
	) -> Result<Self, Error> {
		let ws_client = new_websocket_client(url, None, None).await?;
		Self::new(ws_client, signer, shutdown_tx).await
	}

	pub async fn from_url_with_retry(
		url: &str,
		signer: Arc<SpacewalkSigner>,
		connection_timeout: Duration,
		shutdown_tx: ShutdownSender,
	) -> Result<Self, Error> {
		Self::from_url_and_config_with_retry(
			url,
			signer,
			None,
			None,
			connection_timeout,
			shutdown_tx,
		)
		.await
	}

	pub async fn from_url_and_config_with_retry(
		url: &str,
		signer: Arc<SpacewalkSigner>,
		max_concurrent_requests: Option<usize>,
		max_notifs_per_subscription: Option<usize>,
		connection_timeout: Duration,
		shutdown_tx: ShutdownSender,
	) -> Result<Self, Error> {
		let ws_client = new_websocket_client_with_retry(
			url,
			max_concurrent_requests,
			max_notifs_per_subscription,
			connection_timeout,
		)
		.await?;
		// let ws_client = new_websocket_client(url, None, None).await?;
		Self::new(ws_client, signer, shutdown_tx).await
	}

	// async fn refresh_nonce(&mut self) {
	// 	// For getting the nonce, use latest, possibly non-finalized block.
	// 	// TODO: we might want to wait until the latest block is actually finalized
	// 	// query account info in order to get the nonce value used for communication
	// 	let account_info_query = metadata::storage().system().account(&self.account_id);
	// 	let account_info = self.api.storage().fetch(&account_info_query, None).await;
	//
	// 	let nonce = account_info
	// 		.map(|x| match x {
	// 			Some(x) => x.nonce,
	// 			None => 0,
	// 		})
	// 		.unwrap_or(0);
	//
	// 	log::info!("Refreshing nonce: {}", nonce);
	// 	self.signer.set_nonce(nonce);
	// }

	/// Gets a copy of the signer with a unique nonce
	async fn with_unique_signer<'client, F, R>(
		&mut self,
		call: F,
	) -> Result<TxEvents<SpacewalkRuntime>, Error>
	where
		F: Fn(&SpacewalkSigner) -> R,
		R: Future<
			Output = Result<
				TxProgress<SpacewalkRuntime, OnlineClient<SpacewalkRuntime>>,
				BasicError,
			>,
		>,
	{
		notify_retry::<Error, _, _, _, _, _>(
			|| async {
				match timeout(TRANSACTION_TIMEOUT, async {
					call(&self.signer).await?.wait_for_finalized_success().await
				})
				.await
				{
					Err(_) => {
						log::warn!("Timeout on transaction submission - restart required");
						let _ = self.shutdown_tx.send(Some(()));
						Err(Error::Timeout)
					},
					Ok(x) => Ok(x?),
				}
			},
			|result| async {
				match result.map_err(Into::<Error>::into) {
					Ok(te) => Ok(te),
					Err(err) =>
						if let Some(data) = err.is_invalid_transaction() {
							Err(RetryPolicy::Skip(Error::InvalidTransaction(data)))
						} else if err.is_pool_too_low_priority().is_some() {
							Err(RetryPolicy::Skip(Error::PoolTooLowPriority))
						} else if err.is_block_hash_not_found_error() {
							log::info!("Re-sending transaction after apparent fork");
							Err(RetryPolicy::Skip(Error::BlockHashNotFound))
						} else {
							Err(RetryPolicy::Throw(err))
						},
				}
			},
		)
		.await
	}

	pub async fn get_latest_block_hash(&self) -> Result<Option<H256>, Error> {
		Ok(Some(self.api.rpc().finalized_head().await?))
	}

	/// Subscribe to new parachain blocks.
	pub async fn on_block<F, R>(&self, on_block: F) -> Result<(), Error>
	where
		F: Fn(SpacewalkHeader) -> R,
		R: Future<Output = Result<(), Error>>,
	{
		let mut sub = self.api.rpc().subscribe_finalized_blocks().await?;
		loop {
			on_block(sub.next().await.ok_or(Error::ChannelClosed)??).await?;
		}
	}

	/// Subscription service that should listen forever, only returns if the initial subscription
	/// cannot be established. Calls `on_error` when an error event has been received, or when an
	/// event has been received that failed to be decoded into a raw event.
	///
	/// # Arguments
	/// * `on_error` - callback for decoding errors, is not allowed to take too long
	pub async fn on_event_error<E: Fn(BasicError)>(&self, on_error: E) -> Result<(), Error> {
		let mut sub = self.api.events().subscribe_finalized().await?;

		loop {
			match sub.next().await {
				Some(Err(err)) => on_error(err), // report error
				Some(Ok(_)) => {},               // do nothing
				None => break Ok(()),            // end of stream
			}
		}
	}

	/// Subscription service that should listen forever, only returns if the initial subscription
	/// cannot be established. This function uses two concurrent tasks: one for the event listener,
	/// and one that calls the given callback. This allows the callback to take a long time to
	/// complete without breaking the rpc communication, which could otherwise happen. Still, since
	/// the queue of callbacks is processed sequentially, some care should be taken that the queue
	/// does not overflow. `on_error` is called when the event has successfully been decoded into a
	/// raw_event, but failed to decode into an event of type `T`
	///
	/// # Arguments
	/// * `on_event` - callback for events, is allowed to sometimes take a longer time
	/// * `on_error` - callback for decoding error, is not allowed to take too long
	pub async fn on_event<T, F, R, E>(&self, mut on_event: F, on_error: E) -> Result<(), Error>
	where
		T: StaticEvent + core::fmt::Debug,
		F: FnMut(T) -> R,
		R: Future<Output = ()>,
		E: Fn(SubxtError),
	{
		let mut sub = self.api.events().subscribe_finalized().await?.filter_events::<(T,)>();
		let (tx, mut rx) = futures::channel::mpsc::channel::<T>(32);

		// two tasks: one for event listening and one for callback calling
		futures::future::try_join(
			async move {
				let tx = &tx;
				while let Some(result) = sub.next().fuse().await {
					match result {
						Ok(event_details) => {
							let event = event_details.event;
							log::trace!("event: {:?}", event);
							if tx.clone().send(event).await.is_err() {
								break
							}
						},
						Err(err) => on_error(err.into()),
					}
				}
				Result::<(), _>::Err(Error::ChannelClosed)
			},
			async move {
				loop {
					// block until we receive an event from the other task
					match rx.next().fuse().await {
						Some(event) => {
							on_event(event).await;
						},
						None => return Result::<(), _>::Err(Error::ChannelClosed),
					}
				}
			},
		)
		.await?;

		Ok(())
	}

	/// Emulate the POOL_INVALID_TX error using token transfer extrinsics.
	#[cfg(test)]
	pub async fn get_invalid_tx_error(&self, recipient: AccountId) -> Error {
		let mut signer = self.signer.write().await;

		self.api
			.tx()
			.tokens()
			.transfer(recipient.clone(), Token(DOT), 100)
			.sign_and_submit_then_watch_default(&signer.clone())
			.await
			.unwrap();

		signer.set_nonce(0);

		// now call with outdated nonce
		self.api
			.tx()
			.tokens()
			.transfer(recipient.clone(), Token(DOT), 100)
			.sign_and_submit_then_watch_default(&signer.clone())
			.await
			.unwrap_err()
			.into()
	}

	/// Emulate the POOL_TOO_LOW_PRIORITY error using token transfer extrinsics.
	#[cfg(test)]
	pub async fn get_too_low_priority_error(&self, recipient: AccountId) -> Error {
		let signer = self.signer.write().await;

		// submit tx but don't watch
		self.api
			.tx()
			.tokens()
			.transfer(recipient.clone(), Token(DOT), 100)
			.sign_and_submit_default(&signer.clone())
			.await
			.unwrap();

		// should call with the same nonce
		self.api
			.tx()
			.tokens()
			.transfer(recipient, Token(DOT), 100)
			.sign_and_submit_then_watch_default(&signer.clone())
			.await
			.unwrap_err()
			.into()
	}
}

#[async_trait]
pub trait UtilFuncs {
	/// Gets the current height of the parachain
	async fn get_current_chain_height(&self) -> Result<u32, Error>;

	/// Get the address of the configured signer.
	fn get_account_id(&self) -> &AccountId;
}

#[async_trait]
impl UtilFuncs for SpacewalkParachain {
	async fn get_current_chain_height(&self) -> Result<u32, Error> {
		let height_query = metadata::storage().system().number();
		let height = self.api.storage().fetch(&height_query, None).await?;
		match height {
			Some(height) => Ok(height),
			None => Err(Error::BlockNotFound),
		}
	}

	fn get_account_id(&self) -> &AccountId {
		&self.account_id
	}
}
