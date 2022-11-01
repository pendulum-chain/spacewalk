use crate::{
	conn::{new_websocket_client, new_websocket_client_with_retry},
	metadata,
	metadata::{DispatchError, Event as SpacewalkEvent},
	notify_retry,
	types::*,
	AccountId, Error, RetryPolicy, SpacewalkRuntime, SpacewalkSigner, SubxtError,
};

use async_trait::async_trait;
use futures::{stream::StreamExt, FutureExt, SinkExt};
use std::{future::Future, ops::RangeInclusive, sync::Arc, time::Duration};
use subxt::{
	rpc::JsonValue, BasicError, Client as SubxtClient, ClientBuilder as SubxtClientBuilder, Event,
	Metadata, PolkadotExtrinsicParams, RpcClient, TransactionEvents, TransactionProgress,
};
use tokio::{sync::RwLock, time::timeout};

cfg_if::cfg_if! {
	if #[cfg(feature = "standalone-metadata")]
	{
		const DEFAULT_SPEC_VERSION: RangeInclusive<u32> = 1..=1;
		const DEFAULT_SPEC_NAME: &str = "spacewalk-standalone";
	} else if #[cfg(feature = "90-metadata")] {
		const DEFAULT_SPEC_VERSION: RangeInclusive<u32> = 1..=1;
		const DEFAULT_SPEC_NAME: &str = "spacewalk-standalone";
	} else if #[cfg(feature = "parachain-metadata")] {
		const DEFAULT_SPEC_VERSION: RangeInclusive<u32> = 1..=1;
		const DEFAULT_SPEC_NAME: &str = "pendulum-parachain";
	}
}
const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

type RuntimeApi = metadata::RuntimeApi<SpacewalkRuntime, PolkadotExtrinsicParams<SpacewalkRuntime>>;
pub(crate) type ShutdownSender = tokio::sync::broadcast::Sender<Option<()>>;

#[derive(Clone)]
pub struct SpacewalkParachain {
	ext_client: SubxtClient<SpacewalkRuntime>,
	signer: Arc<RwLock<SpacewalkSigner>>,
	account_id: AccountId,
	api: Arc<RuntimeApi>,
	shutdown_tx: ShutdownSender,
	metadata: Arc<Metadata>,
}

impl SpacewalkParachain {
	pub async fn new<P: Into<RpcClient>>(
		rpc_client: P,
		signer: SpacewalkSigner,
		shutdown_tx: ShutdownSender,
	) -> Result<Self, Error> {
		let account_id = signer.account_id().clone();
		let ext_client = SubxtClientBuilder::new().set_client(rpc_client).build().await?;
		let api: RuntimeApi = ext_client.clone().to_runtime_api();
		let metadata = Arc::new(ext_client.rpc().metadata().await?);

		let runtime_version = ext_client.rpc().runtime_version(None).await?;
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

		let parachain_rpc = Self {
			ext_client,
			api: Arc::new(api),
			shutdown_tx,
			metadata,
			signer: Arc::new(RwLock::new(signer)),
			account_id,
		};
		parachain_rpc.refresh_nonce().await;
		Ok(parachain_rpc)
	}

	pub async fn from_url(
		url: &str,
		signer: SpacewalkSigner,
		shutdown_tx: ShutdownSender,
	) -> Result<Self, Error> {
		let ws_client = new_websocket_client(url, None, None).await?;
		Self::new(ws_client, signer, shutdown_tx).await
	}

	pub async fn from_url_with_retry(
		url: &str,
		signer: SpacewalkSigner,
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
		signer: SpacewalkSigner,
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
		Self::new(ws_client, signer, shutdown_tx).await
	}

	async fn refresh_nonce(&self) {
		let mut signer = self.signer.write().await;
		// For getting the nonce, use latest, possibly non-finalized block.
		// TODO: we might want to wait until the latest block is actually finalized
		// query account info in order to get the nonce value used for communication
		let account_info = self
			.api
			.storage()
			.system()
			.account(&self.account_id, None)
			.await
			.map(|x| x.nonce)
			.unwrap_or(0);

		log::info!("Refreshing nonce: {}", account_info);
		signer.set_nonce(account_info);
	}

	/// Gets a copy of the signer with a unique nonce
	async fn with_unique_signer<'client, F, R>(
		&self,
		call: F,
	) -> Result<TransactionEvents<'client, SpacewalkRuntime, SpacewalkEvent>, Error>
	where
		F: Fn(SpacewalkSigner) -> R,
		R: Future<
			Output = Result<
				TransactionProgress<'client, SpacewalkRuntime, DispatchError, SpacewalkEvent>,
				BasicError,
			>,
		>,
	{
		notify_retry::<Error, _, _, _, _, _>(
			|| async {
				let signer = {
					let mut signer = self.signer.write().await;
					// return the current value, increment afterwards
					let cloned_signer = signer.clone();
					signer.increment_nonce();
					cloned_signer
				};
				match timeout(TRANSACTION_TIMEOUT, async {
					call(signer).await?.wait_for_finalized_success().await
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
							self.refresh_nonce().await;
							Err(RetryPolicy::Skip(Error::InvalidTransaction(data)))
						} else if err.is_pool_too_low_priority().is_some() {
							self.refresh_nonce().await;
							Err(RetryPolicy::Skip(Error::PoolTooLowPriority))
						} else if err.is_block_hash_not_found_error() {
							self.refresh_nonce().await;
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
		Ok(Some(self.ext_client.rpc().finalized_head().await?))
	}

	/// Subscribe to new parachain blocks.
	pub async fn on_block<F, R>(&self, on_block: F) -> Result<(), Error>
	where
		F: Fn(SpacewalkHeader) -> R,
		R: Future<Output = Result<(), Error>>,
	{
		let mut sub = self.ext_client.rpc().subscribe_finalized_blocks().await?;
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
		T: Event + core::fmt::Debug,
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
		let head = self.get_latest_block_hash().await?;
		Ok(self.api.storage().system().number(head).await?)
	}

	fn get_account_id(&self) -> &AccountId {
		&self.account_id
	}
}

pub type IssueId = H256;

#[async_trait]
pub trait IssuePallet {
	// async fn request_issue(
	// 	&self,
	// 	amount: u128,
	// 	asset: CurrencyId,
	// 	vault_id: VaultId<AccountId, CurrencyId>,
	// 	public_network: bool
	// ) -> Result<(),Error> ;

	async fn execute_issue(
		&self,
		issue_id: IssueId,
		tx_envelope_xdr_encoded: &[u8],
		envelopes_xdr_encoded: &[u8],
		tx_set_xdr_encoded: &[u8],
	) -> Result<(), Error>;

	// async fn cancel_issue(
	// 	&self,
	// 	issue_id:IssueId
	// ) -> Result<(),Error> ;
	//
	// async fn set_issue_period(
	// 	&self,
	// 	period: u32
	// ) -> Result<(),Error> ;
}

#[async_trait]
impl IssuePallet for SpacewalkParachain {
	async fn execute_issue(
		&self,
		issue_id: IssueId,
		tx_envelope_xdr_encoded: &[u8],
		envelopes_xdr_encoded: &[u8],
		tx_set_xdr_encoded: &[u8],
	) -> Result<(), Error> {
		self.with_unique_signer(|signer| async move {
			self.api
				.tx()
				.issue()
				.execute_issue(
					issue_id,
					tx_envelope_xdr_encoded.to_vec(),
					envelopes_xdr_encoded.to_vec(),
					tx_set_xdr_encoded.to_vec(),
				)
				.sign_and_submit_then_watch_default(&signer)
				.await
		})
		.await?;
		Ok(())
	}
}

/*
#[async_trait]
pub trait SpacewalkPallet {
	async fn report_stellar_transaction(&self, tx_envelope_xdr: &Vec<u8>) -> Result<(), Error>;

	async fn redeem(
		&self,
		asset_code: &Vec<u8>,
		asset_issuer: &Vec<u8>,
		amount: u128,
		stellar_vault_pubkey: [u8; 32],
	) -> Result<(), Error>;
}

#[async_trait]
impl SpacewalkPallet for SpacewalkParachain {
	async fn report_stellar_transaction(&self, tx_envelope_xdr: &Vec<u8>) -> Result<(), Error> {
		self.with_unique_signer(|signer| async move {
			self.api
				.tx()
				.spacewalk() // assume that spacewalk pallet is registered in connected chain
				.report_stellar_transaction(tx_envelope_xdr.to_vec()) // spacewalk pallet offers extrinsic `report_stellar_transaction`
				.sign_and_submit_then_watch_default(&signer)
				.await
		})
		.await?;
		Ok(())
	}

	async fn redeem(
		&self,
		asset_code: &Vec<u8>,
		asset_issuer: &Vec<u8>,
		amount: u128,
		stellar_vault_pubkey: [u8; 32],
	) -> Result<(), Error> {
		self.with_unique_signer(|signer| async move {
			self.api
				.tx()
				.spacewalk() // assume that spacewalk pallet is registered in connected chain
				.redeem(
					asset_code.to_vec(),
					asset_issuer.to_vec(),
					amount,
					stellar_vault_pubkey.clone().to_vec(),
				) // spacewalk pallet offers extrinsic `redeem`
				.sign_and_submit_then_watch_default(&signer)
				.await
		})
		.await?;
		Ok(())
	}
}
*/
