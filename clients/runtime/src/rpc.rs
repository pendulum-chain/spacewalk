use std::{future::Future, ops::RangeInclusive, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::{stream::StreamExt, FutureExt, SinkExt};
use jsonrpsee::core::{client::Client, JsonValue};
use log::log;
use sp_arithmetic::FixedPointNumber;
use sp_runtime::FixedU128;
use subxt::{
	blocks::ExtrinsicEvents,
	client::OnlineClient,
	events::{EventDetails, StaticEvent},
	metadata::DecodeWithMetadata,
	rpc::{rpc_params, RpcClient, RpcClientT},
	storage::{address::Yes, StorageAddress},
	tx::{PolkadotExtrinsicParams, Signer, TxPayload, TxProgress},
	Error as BasicError, Metadata,
};
use tokio::{
	sync::{Mutex, RwLock},
	time::timeout,
};

use module_oracle_rpc_runtime_api::BalanceWrapper;

use crate::{
	conn::{new_websocket_client, new_websocket_client_with_retry},
	metadata,
	metadata::{DispatchError, Event as SpacewalkEvent},
	notify_retry,
	types::*,
	AccountId, Error, RetryPolicy, SpacewalkRuntime, SpacewalkSigner, SubxtError,
};

pub type UnsignedFixedPoint = FixedU128;

cfg_if::cfg_if! {
	if #[cfg(feature = "standalone-metadata")] {
		const DEFAULT_SPEC_VERSION: RangeInclusive<u32> = 1..=1;
		pub const DEFAULT_SPEC_NAME: &str = "spacewalk-standalone";
		// The prefix for the testchain is 42
		pub const SS58_PREFIX: u16 = 42;
	} else if #[cfg(feature = "parachain-metadata")] {
		const DEFAULT_SPEC_VERSION: RangeInclusive<u32> = 1..=1;
		pub const DEFAULT_SPEC_NAME: &str = "pendulum-parachain";
		// The prefix for pendulum is 56
		pub const SS58_PREFIX: u16 = 56;
	}
}

// timeout before retrying parachain calls (5 minutes)
const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

// number of storage entries to fetch at a time
const DEFAULT_PAGE_SIZE: u32 = 10;

pub(crate) type ShutdownSender = tokio::sync::broadcast::Sender<()>;

#[derive(Clone)]
pub struct SpacewalkParachain {
	signer: Arc<RwLock<SpacewalkSigner>>,
	account_id: AccountId,
	api: OnlineClient<SpacewalkRuntime>,
	shutdown_tx: ShutdownSender,
	metadata: Arc<Metadata>,
}

impl SpacewalkParachain {
	pub async fn new(
		rpc_client: Client,
		signer: Arc<RwLock<SpacewalkSigner>>,
		shutdown_tx: ShutdownSender,
	) -> Result<Self, Error> {
		let account_id = signer.read().await.account_id().clone();
		let api = OnlineClient::<SpacewalkRuntime>::from_rpc_client(Arc::new(rpc_client)).await?;
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
		signer: Arc<RwLock<SpacewalkSigner>>,
		shutdown_tx: ShutdownSender,
	) -> Result<Self, Error> {
		let ws_client = new_websocket_client(url, None, None).await?;
		Self::new(ws_client, signer, shutdown_tx).await
	}

	pub async fn from_url_with_retry(
		url: &str,
		signer: Arc<RwLock<SpacewalkSigner>>,
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
		signer: Arc<RwLock<SpacewalkSigner>>,
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

	async fn with_retry<Call>(&self, call: Call) -> Result<ExtrinsicEvents<SpacewalkRuntime>, Error>
	where
		Call: TxPayload,
	{
		notify_retry::<Error, _, _, _, _, _>(
			|| async {
				let signer = self.signer.read().await;
				match timeout(TRANSACTION_TIMEOUT, async {
					let tx_progress =
						self.api.tx().sign_and_submit_then_watch_default(&call, &*signer).await?;
					tx_progress.wait_for_finalized_success().await
				})
				.await
				{
					Err(_) => {
						log::warn!("Timeout on transaction submission - restart required");
						let _ = self.shutdown_tx.send(());
						Err(Error::Timeout)
					},
					Ok(x) => Ok(x?),
				}
			},
			|result| async {
				match result.map_err(Into::<Error>::into) {
					Ok(te) => Ok(te),
					Err(err) => Err(RetryPolicy::Throw(err)),
				}
			},
		)
		.await
	}

	async fn query_finalized<Address>(
		&self,
		address: Address,
	) -> Result<Option<<Address::Target as DecodeWithMetadata>::Target>, Error>
	where
		Address: StorageAddress<IsFetchable = Yes>,
	{
		let hash = self.get_finalized_block_hash().await?;
		Ok(self.api.storage().fetch(&address, hash).await?)
	}

	async fn query_finalized_or_error<Address>(
		&self,
		address: Address,
	) -> Result<<Address::Target as DecodeWithMetadata>::Target, Error>
	where
		Address: StorageAddress<IsFetchable = Yes>,
	{
		self.query_finalized(address).await?.ok_or(Error::StorageItemNotFound)
	}

	async fn query_finalized_or_default<Address>(
		&self,
		address: Address,
	) -> Result<<Address::Target as DecodeWithMetadata>::Target, Error>
	where
		Address: StorageAddress<IsFetchable = Yes, IsDefaultable = Yes>,
	{
		let hash = self.get_finalized_block_hash().await?;
		Ok(self.api.storage().fetch_or_default(&address, hash).await?)
	}

	pub async fn get_finalized_block_hash(&self) -> Result<Option<H256>, Error> {
		Ok(Some(self.api.rpc().finalized_head().await?))
	}

	/// Subscribe to new parachain blocks.
	pub async fn on_block<F, R>(&self, on_block: F) -> Result<(), Error>
	where
		F: Fn(SpacewalkHeader) -> R,
		R: Future<Output = Result<(), Error>>,
	{
		let mut sub = self.api.rpc().subscribe_finalized_block_headers().await?;
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
		let mut sub = self.api.blocks().subscribe_finalized().await?;

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
		let mut sub = self.api.blocks().subscribe_finalized().await?;
		let (tx, mut rx) = futures::channel::mpsc::channel(32);

		// two tasks: one for event listening and one for callback calling
		futures::future::try_join(
			async move {
				let tx = &tx;
				while let Some(result) = sub.next().fuse().await {
					let block = result?;
					let events = block.events().await?;
					for event in events.iter() {
						match event {
							Ok(event) => {
								// Try to convert to target event
								let target_event = event.as_event::<T>();
								if let Ok(Some(target_event)) = target_event {
									log::trace!("event: {:?}", target_event);
									if tx.clone().send(target_event).await.is_err() {
										break
									}
								}
							},
							Err(err) => on_error(err.into()),
						}
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

	fn is_this_vault(&self, vault_id: &VaultId) -> bool;
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

	fn is_this_vault(&self, vault_id: &VaultId) -> bool {
		&vault_id.account_id == self.get_account_id()
	}

	fn get_account_id(&self) -> &AccountId {
		&self.account_id
	}
}

#[async_trait]
pub trait VaultRegistryPallet {
	async fn get_vault(&self, vault_id: &VaultId) -> Result<SpacewalkVault, Error>;

	async fn get_vaults_by_account_id(&self, account_id: &AccountId)
		-> Result<Vec<VaultId>, Error>;

	async fn get_all_vaults(&self) -> Result<Vec<SpacewalkVault>, Error>;

	async fn register_vault(&self, vault_id: &VaultId, collateral: u128) -> Result<(), Error>;

	async fn deposit_collateral(&self, vault_id: &VaultId, amount: u128) -> Result<(), Error>;

	async fn withdraw_collateral(&self, vault_id: &VaultId, amount: u128) -> Result<(), Error>;

	async fn get_public_key(&self) -> Result<Option<StellarPublicKey>, Error>;

	async fn register_public_key(&mut self, public_key: StellarPublicKey) -> Result<(), Error>;

	async fn get_required_collateral_for_wrapped(
		&self,
		amount_btc: u128,
		collateral_currency: CurrencyId,
	) -> Result<u128, Error>;

	async fn get_required_collateral_for_vault(&self, vault_id: VaultId) -> Result<u128, Error>;

	async fn get_vault_total_collateral(&self, vault_id: VaultId) -> Result<u128, Error>;

	async fn get_collateralization_from_vault(
		&self,
		vault_id: VaultId,
		only_issued: bool,
	) -> Result<u128, Error>;
}

#[async_trait]
impl VaultRegistryPallet for SpacewalkParachain {
	/// Fetch a specific vault by ID.
	///
	/// # Arguments
	/// * `vault_id` - account ID of the vault
	///
	/// # Errors
	/// * `VaultNotFound` - if the rpc returned a default value rather than the vault we want
	/// * `VaultLiquidated` - if the vault is liquidated
	async fn get_vault(&self, vault_id: &VaultId) -> Result<SpacewalkVault, Error> {
		let query = metadata::storage().vault_registry().vaults(&vault_id.clone());

		match self.query_finalized(query).await? {
			Some(SpacewalkVault { status: VaultStatus::Liquidated, .. }) =>
				Err(Error::VaultLiquidated),
			Some(vault) if &vault.id == vault_id => Ok(vault),
			_ => Err(Error::VaultNotFound),
		}
	}

	async fn get_vaults_by_account_id(
		&self,
		account_id: &AccountId,
	) -> Result<Vec<VaultId>, Error> {
		let head = self.get_finalized_block_hash().await?;
		let result = self
			.api
			.rpc()
			.request("vaultRegistry_getVaultsByAccountId", rpc_params![account_id, head])
			.await?;

		Ok(result)
	}

	/// Fetch all active vaults.
	async fn get_all_vaults(&self) -> Result<Vec<SpacewalkVault>, Error> {
		let mut vaults = Vec::new();
		let head = self.get_finalized_block_hash().await?;
		let key_addr = metadata::storage().vault_registry().vaults_root();

		let mut iter = self.api.storage().iter(key_addr, DEFAULT_PAGE_SIZE, head).await?;
		while let Some((_, account)) = iter.next().await? {
			if let VaultStatus::Active(..) = account.status {
				vaults.push(account);
			}
		}
		Ok(vaults)
	}

	/// Submit extrinsic to register a vault.
	///
	/// # Arguments
	/// * `collateral` - deposit
	/// * `public_key` - Bitcoin public key
	async fn register_vault(&self, vault_id: &VaultId, collateral: u128) -> Result<(), Error> {
		// TODO: check MinimumDeposit
		if collateral == 0 {
			return Err(Error::InsufficientFunds)
		}

		let register_vault_tx = metadata::tx()
			.vault_registry()
			.register_vault(vault_id.currencies.clone(), collateral);

		self.with_retry(register_vault_tx).await?;
		Ok(())
	}

	/// Locks additional collateral as a security against stealing the
	/// Bitcoin locked with it.
	///
	/// # Arguments
	/// * `amount` - the amount of extra collateral to lock
	async fn deposit_collateral(&self, vault_id: &VaultId, amount: u128) -> Result<(), Error> {
		let deposit_collateral_tx = metadata::tx()
			.vault_registry()
			.deposit_collateral(vault_id.currencies.clone(), amount);

		self.with_retry(deposit_collateral_tx).await?;
		Ok(())
	}

	/// Withdraws `amount` of the collateral from the amount locked by
	/// the vault corresponding to the origin account
	/// The collateral left after withdrawal must be more than MinimumCollateralVault
	/// and above the SecureCollateralThreshold. Collateral that is currently
	/// being used to back issued tokens remains locked until the Vault
	/// is used for a redeem request (full release can take multiple redeem requests).
	///
	/// # Arguments
	/// * `amount` - the amount of collateral to withdraw
	async fn withdraw_collateral(&self, vault_id: &VaultId, amount: u128) -> Result<(), Error> {
		let withdraw_collateral_tx = metadata::tx()
			.vault_registry()
			.withdraw_collateral(vault_id.currencies.clone(), amount);

		self.with_retry(withdraw_collateral_tx).await?;
		Ok(())
	}

	async fn get_public_key(&self) -> Result<Option<StellarPublicKey>, Error> {
		let query = metadata::storage()
			.vault_registry()
			.vault_stellar_public_key(self.get_account_id());

		self.query_finalized(query).await
	}

	/// Update the default BTC public key for the vault corresponding to the signer.
	///
	/// # Arguments
	/// * `public_key` - the new public key of the vault
	async fn register_public_key(&mut self, public_key: StellarPublicKey) -> Result<(), Error> {
		let register_public_key_tx =
			metadata::tx().vault_registry().register_public_key(public_key.clone());

		self.with_retry(register_public_key_tx).await?;

		Ok(())
	}

	/// Custom RPC that calculates the exact collateral required to cover the BTC amount.
	///
	/// # Arguments
	/// * `amount_btc` - amount of btc to cover
	async fn get_required_collateral_for_wrapped(
		&self,
		amount_btc: u128,
		collateral_currency: CurrencyId,
	) -> Result<u128, Error> {
		let head = self.get_finalized_block_hash().await?;
		let result: BalanceWrapper<_> = self
			.api
			.rpc()
			.request(
				"vaultRegistry_getRequiredCollateralForWrapped",
				rpc_params![BalanceWrapper { amount: amount_btc }, collateral_currency, head],
			)
			.await?;

		Ok(result.amount)
	}

	/// Get the amount of collateral required for the given vault to be at the
	/// current SecureCollateralThreshold with the current exchange rate
	async fn get_required_collateral_for_vault(&self, vault_id: VaultId) -> Result<u128, Error> {
		let head = self.get_finalized_block_hash().await?;
		let result: BalanceWrapper<_> = self
			.api
			.rpc()
			.request("vaultRegistry_getRequiredCollateralForVault", rpc_params![vault_id, head])
			.await?;

		Ok(result.amount)
	}

	async fn get_vault_total_collateral(&self, vault_id: VaultId) -> Result<u128, Error> {
		let head = self.get_finalized_block_hash().await?;
		let result: BalanceWrapper<_> = self
			.api
			.rpc()
			.request("vaultRegistry_getVaultTotalCollateral", rpc_params![vault_id, head])
			.await?;

		Ok(result.amount)
	}

	async fn get_collateralization_from_vault(
		&self,
		vault_id: VaultId,
		only_issued: bool,
	) -> Result<u128, Error> {
		let head = self.get_finalized_block_hash().await?;
		let result: UnsignedFixedPoint = self
			.api
			.rpc()
			.request(
				"vaultRegistry_getCollateralizationFromVault",
				rpc_params![vault_id, only_issued, head],
			)
			.await?;

		Ok(result.into_inner())
	}
}

#[async_trait]
pub trait CollateralBalancesPallet {
	async fn get_free_balance(&self, currency_id: CurrencyId) -> Result<Balance, Error>;

	async fn get_free_balance_for_id(
		&self,
		id: AccountId,
		currency_id: CurrencyId,
	) -> Result<Balance, Error>;

	async fn get_reserved_balance(&self, currency_id: CurrencyId) -> Result<Balance, Error>;

	async fn get_reserved_balance_for_id(
		&self,
		id: AccountId,
		currency_id: CurrencyId,
	) -> Result<Balance, Error>;

	async fn transfer_to(
		&self,
		recipient: &AccountId,
		amount: u128,
		currency_id: CurrencyId,
	) -> Result<(), Error>;
}

#[async_trait]
impl CollateralBalancesPallet for SpacewalkParachain {
	async fn get_free_balance(&self, currency_id: CurrencyId) -> Result<Balance, Error> {
		Ok(Self::get_free_balance_for_id(self, self.account_id.clone(), currency_id).await?)
	}

	async fn get_free_balance_for_id(
		&self,
		id: AccountId,
		currency_id: CurrencyId,
	) -> Result<Balance, Error> {
		let head = self.get_finalized_block_hash().await?;
		let query = metadata::storage().tokens().accounts(&id, &currency_id);

		let result = self.api.storage().fetch(&query, head).await?;
		Ok(result.map(|x| x.free).unwrap_or_default())
	}

	async fn get_reserved_balance(&self, currency_id: CurrencyId) -> Result<Balance, Error> {
		Ok(Self::get_reserved_balance_for_id(self, self.account_id.clone(), currency_id).await?)
	}

	async fn get_reserved_balance_for_id(
		&self,
		id: AccountId,
		currency_id: CurrencyId,
	) -> Result<Balance, Error> {
		let head = self.get_finalized_block_hash().await?;
		let query = metadata::storage().tokens().accounts(&id, &currency_id);

		let result = self.api.storage().fetch(&query, head).await?;
		Ok(result.map(|x| x.reserved).unwrap_or_default())
	}

	async fn transfer_to(
		&self,
		recipient: &AccountId,
		amount: u128,
		currency_id: CurrencyId,
	) -> Result<(), Error> {
		let transfer_tx = metadata::tx().tokens().transfer(
			subxt::ext::sp_runtime::MultiAddress::Id(recipient.clone()),
			currency_id,
			amount,
		);

		let signer = self.signer.read().await;

		self.api.tx().sign_and_submit_then_watch_default(&transfer_tx, &*signer).await?;
		Ok(())
	}
}
