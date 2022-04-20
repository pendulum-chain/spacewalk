use crate::{
    conn::{new_websocket_client, new_websocket_client_with_retry},
    metadata,
    metadata::DispatchError,
    notify_retry,
    types::*,
    AccountId, Error, SpacewalkRuntime, SpacewalkSigner, RetryPolicy, SubxtError,
};

use async_trait::async_trait;
use futures::{stream::StreamExt, FutureExt, SinkExt};
use std::{future::Future, sync::Arc, time::Duration};
use subxt::{
    BasicError, Client as SubxtClient, ClientBuilder as SubxtClientBuilder, DefaultExtra, Event, EventSubscription,
    EventsDecoder, Metadata, RpcClient, Signer, TransactionEvents, TransactionProgress,
};
use tokio::sync::RwLock;

cfg_if::cfg_if! {
    if #[cfg(feature = "standalone-metadata")] {
        const DEFAULT_SPEC_VERSION: u32 = 1;
    } else if #[cfg(feature = "parachain-metadata-kintsugi")] {
        const DEFAULT_SPEC_VERSION: u32 = 11;
    } else if #[cfg(feature = "parachain-metadata-testnet")] {
        const DEFAULT_SPEC_VERSION: u32 = 2;
    }
}

type RuntimeApi = metadata::RuntimeApi<SpacewalkRuntime, DefaultExtra<SpacewalkRuntime>>;

#[derive(Clone)]
pub struct SpacewalkParachain {
    rpc_client: RpcClient,
    ext_client: SubxtClient<SpacewalkRuntime>,
    signer: Arc<RwLock<SpacewalkSigner>>,
    account_id: AccountId,
    api: Arc<RuntimeApi>,
    metadata: Arc<Metadata>,
}

impl SpacewalkParachain {
    pub async fn new<P: Into<RpcClient>>(rpc_client: P, signer: SpacewalkSigner) -> Result<Self, Error> {
        let account_id = signer.account_id().clone();
        let rpc_client = rpc_client.into();
        let ext_client = SubxtClientBuilder::new().set_client(rpc_client.clone()).build().await?;
        let api: RuntimeApi = ext_client.clone().to_runtime_api();
        let metadata = Arc::new(ext_client.rpc().metadata().await?);

        let runtime_version = ext_client.rpc().runtime_version(None).await?;
        if runtime_version.spec_version == DEFAULT_SPEC_VERSION {
            log::info!("spec_version={}", runtime_version.spec_version);
            log::info!("transaction_version={}", runtime_version.transaction_version);
        } else {
            return Err(Error::InvalidSpecVersion(
                DEFAULT_SPEC_VERSION,
                runtime_version.spec_version,
            ));
        }

        let parachain_rpc = Self {
            rpc_client,
            ext_client,
            api: Arc::new(api),
            metadata,
            signer: Arc::new(RwLock::new(signer)),
            account_id,
        };
        parachain_rpc.refresh_nonce().await;
        Ok(parachain_rpc)
    }

    pub async fn from_url(url: &str, signer: SpacewalkSigner) -> Result<Self, Error> {
        let ws_client = new_websocket_client(url, None, None).await?;
        Self::new(ws_client, signer).await
    }

    pub async fn from_url_with_retry(
        url: &str,
        signer: SpacewalkSigner,
        connection_timeout: Duration,
    ) -> Result<Self, Error> {
        Self::from_url_and_config_with_retry(url, signer, None, None, connection_timeout).await
    }

    pub async fn from_url_and_config_with_retry(
        url: &str,
        signer: SpacewalkSigner,
        max_concurrent_requests: Option<usize>,
        max_notifs_per_subscription: Option<usize>,
        connection_timeout: Duration,
    ) -> Result<Self, Error> {
        let ws_client = new_websocket_client_with_retry(
            url,
            max_concurrent_requests,
            max_notifs_per_subscription,
            connection_timeout,
        )
        .await?;
        Self::new(ws_client, signer).await
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
            .account(self.account_id.clone(), None)
            .await
            .map(|x| x.nonce)
            .unwrap_or(0);

        log::info!("Refreshing nonce: {}", account_info);
        signer.set_nonce(account_info);
    }

    /// Gets a copy of the signer with a unique nonce
    async fn with_unique_signer<'client, F, R>(&self, call: F) -> Result<TransactionEvents<SpacewalkRuntime>, Error>
    where
        F: Fn(SpacewalkSigner) -> R,
        R: Future<Output = Result<TransactionProgress<'client, SpacewalkRuntime, DispatchError>, BasicError>>,
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
                Ok(call(signer).await?.wait_for_finalized_success().await?)
            },
            |result| async {
                match result.map_err(Into::<Error>::into) {
                    Ok(te) => Ok(te),
                    Err(err) => {
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
                        }
                    }
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
        let sub = self.ext_client.rpc().subscribe_finalized_events().await?;
        let decoder = EventsDecoder::<SpacewalkRuntime>::new((*self.metadata).clone());

        let mut sub = EventSubscription::<SpacewalkRuntime>::new(sub, &decoder);
        loop {
            match sub.next().await {
                Some(Err(err)) => on_error(err), // report error
                Some(Ok(_)) => {}                // do nothing
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
        let sub = self.ext_client.rpc().subscribe_finalized_events().await?;
        let decoder = EventsDecoder::<SpacewalkRuntime>::new((*self.metadata).clone());

        let mut sub = EventSubscription::<SpacewalkRuntime>::new(sub, &decoder);
        sub.filter_event::<T>();

        let (tx, mut rx) = futures::channel::mpsc::channel::<T>(32);

        // two tasks: one for event listening and one for callback calling
        futures::future::try_join(
            async move {
                let tx = &tx;
                while let Some(result) = sub.next().fuse().await {
                    if let Ok(raw_event) = result {
                        log::trace!("raw event: {:?}", raw_event);
                        let decoded = T::decode(&mut &raw_event.data[..]);
                        match decoded {
                            Ok(event) => {
                                log::trace!("decoded event: {:?}", event);
                                // send the event to the other task
                                if tx.clone().send(event).await.is_err() {
                                    break;
                                }
                            }
                            Err(err) => {
                                on_error(err.into());
                            }
                        };
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
                        }
                        None => {
                            return Result::<(), _>::Err(Error::ChannelClosed);
                        }
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
            .sign_and_submit_then_watch(&signer.clone())
            .await
            .unwrap();

        signer.set_nonce(0);

        // now call with outdated nonce
        self.api
            .tx()
            .tokens()
            .transfer(recipient.clone(), Token(DOT), 100)
            .sign_and_submit_then_watch(&signer.clone())
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
            .sign_and_submit(&signer.clone())
            .await
            .unwrap();

        // should call with the same nonce
        self.api
            .tx()
            .tokens()
            .transfer(recipient, Token(DOT), 100)
            .sign_and_submit_then_watch(&signer.clone())
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
                .sign_and_submit_then_watch(&signer)
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
                    stellar_vault_pubkey.clone(),
                ) // spacewalk pallet offers extrinsic `redeem`
                .sign_and_submit_then_watch(&signer)
                .await
        })
        .await?;
        Ok(())
    }
}
