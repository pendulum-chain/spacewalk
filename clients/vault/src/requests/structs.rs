use crate::{
	metrics::update_stellar_metrics,
	oracle::{OracleAgent, Proof},
	system::VaultData,
	ArcRwLock, Error,
};
use primitives::{stellar::PublicKey, CurrencyId};
use runtime::{
	Error as EnrichedError, OraclePallet, Recoverability, RedeemPallet, ReplacePallet, RetryPolicy,
	SecurityPallet, SpacewalkRedeemRequest, SpacewalkReplaceRequest, StellarPublicKeyRaw,
	StellarRelayPallet, UtilFuncs, VaultId, VaultRegistryPallet, H256,
};
use sp_runtime::traits::StaticLookup;
use std::{convert::TryInto, sync::Arc, time::Duration};
use stellar_relay_lib::sdk::{Asset, TransactionEnvelope, XdrCodec};
use tokio::sync::RwLock;
use wallet::{Slot, StellarWallet, TransactionResponse};

#[derive(Debug, Clone, PartialEq)]
struct Deadline {
	parachain: u32,
}

#[derive(Debug, Copy, Clone)]
pub enum RequestType {
	Redeem,
	Replace,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Request {
	hash: H256,
	/// Deadline (unit: active block number) after which payments will no longer be attempted.
	deadline: Option<Deadline>,
	amount: u128,
	asset: Asset,
	currency: CurrencyId,
	stellar_address: StellarPublicKeyRaw,
	request_type: RequestType,
	vault_id: VaultId,
	fee_budget: Option<u128>,
}

/// implement getters
impl Request {
	pub fn hash(&self) -> H256 {
		self.hash
	}

	pub fn hash_inner(&self) -> [u8; 32] {
		self.hash.0
	}

	pub fn amount(&self) -> u128 {
		self.amount
	}

	pub fn asset(&self) -> Asset {
		self.asset.clone()
	}

	pub fn stellar_address(&self) -> StellarPublicKeyRaw {
		self.stellar_address
	}

	pub fn request_type(&self) -> RequestType {
		self.request_type
	}

	pub fn vault_id(&self) -> &VaultId {
		&self.vault_id
	}
}

// other public methods
impl Request {
	/// Constructs a Request for the given SpacewalkRedeemRequest
	pub fn from_redeem_request(
		hash: H256,
		request: SpacewalkRedeemRequest,
		payment_margin: Duration,
	) -> Result<Request, Error> {
		// Convert the currency ID contained in the request to a Stellar asset and store both
		// in the request struct for convenience
		let asset =
			primitives::AssetConversion::lookup(*request.asset).map_err(|_| Error::LookupError)?;

		Ok(Request {
			hash,
			deadline: Some(Self::calculate_deadline(
				request.opentime,
				request.period,
				payment_margin,
			)?),
			amount: request.amount,
			asset,
			currency: *request.asset,
			stellar_address: request.stellar_address,
			request_type: RequestType::Redeem,
			vault_id: request.vault,
			fee_budget: Some(request.transfer_fee),
		})
	}

	/// Constructs a Request for the given SpacewalkReplaceRequest
	pub fn from_replace_request(
		hash: H256,
		request: SpacewalkReplaceRequest,
		payment_margin: Duration,
	) -> Result<Request, Error> {
		// Convert the currency ID contained in the request to a Stellar asset and store both
		// in the request struct for convenience
		let asset =
			primitives::AssetConversion::lookup(*request.asset).map_err(|_| Error::LookupError)?;

		Ok(Request {
			hash,
			deadline: Some(Self::calculate_deadline(
				request.accept_time,
				request.period,
				payment_margin,
			)?),
			amount: request.amount,
			asset,
			currency: *request.asset,
			stellar_address: request.stellar_address,
			request_type: RequestType::Replace,
			vault_id: request.old_vault,
			fee_budget: None,
		})
	}

	/// Makes the stellar transfer and executes the request
	pub async fn pay_and_execute<
		P: ReplacePallet
			+ StellarRelayPallet
			+ RedeemPallet
			+ SecurityPallet
			+ VaultRegistryPallet
			+ OraclePallet
			+ UtilFuncs
			+ Clone
			+ Send
			+ Sync,
	>(
		&self,
		parachain_rpc: P,
		vault: VaultData,
		oracle_agent: ArcRwLock<OracleAgent>,
	) -> Result<(), Error> {
		// ensure the deadline has not expired yet
		if let Some(ref deadline) = self.deadline {
			if parachain_rpc.get_current_active_block_number().await? >= deadline.parachain {
				return Err(Error::DeadlineExpired);
			}
		}

		let response = self.transfer_stellar_asset(vault.stellar_wallet.clone()).await?;
		let tx_env = response.to_envelope()?;

		let proof = oracle_agent.read().await.get_proof(response.ledger as Slot).await?;

		let _ = update_stellar_metrics(&vault, &parachain_rpc).await;
		self.execute(parachain_rpc, tx_env, proof).await
	}

	/// Executes the request. Upon failure it will retry again.
	pub(crate) async fn execute<P: ReplacePallet + RedeemPallet>(
		&self,
		parachain_rpc: P,
		tx_env: TransactionEnvelope,
		proof: Proof,
	) -> Result<(), Error> {
		// select the execute function based on request_type
		let execute = match self.request_type {
			RequestType::Redeem => RedeemPallet::execute_redeem,
			RequestType::Replace => ReplacePallet::execute_replace,
		};

		// Encode the proof components
		let tx_env_encoded = tx_env.to_base64_xdr();
		let (scp_envelopes_encoded, tx_set_encoded) = proof.encode();

		// Retry until success or timeout, explicitly handle the cases
		// where the redeem has expired or the rpc has disconnected
		runtime::notify_retry(
			|| {
				(execute)(
					&parachain_rpc,
					self.hash,
					tx_env_encoded.as_slice(),
					scp_envelopes_encoded.as_bytes(),
					tx_set_encoded.as_bytes(),
				)
			},
			|result| async {
				match result.map_err(Into::<EnrichedError>::into) {
					Ok(ok) => Ok(ok),
					Err(err) => match err.is_invalid_transaction() {
						Some(Recoverability::Recoverable(data)) =>
							Err(RetryPolicy::Skip(EnrichedError::InvalidTransaction(data))),
						Some(Recoverability::Unrecoverable(data)) =>
							Err(RetryPolicy::Throw(EnrichedError::InvalidTransaction(data))),
						None => {
							// Handle other errors
							if err.is_pool_too_low_priority() {
								Err(RetryPolicy::Skip(EnrichedError::PoolTooLowPriority))
							} else if err.is_rpc_disconnect_error() {
								Err(RetryPolicy::Throw(err))
							} else if err.is_block_hash_not_found_error() {
								Err(RetryPolicy::Skip(EnrichedError::BlockHashNotFound))
							} else if err.is_timeout_error() {
								Err(RetryPolicy::Skip(EnrichedError::Timeout))
							} else {
								Err(RetryPolicy::Throw(err))
							}
						},
					},
				}
			},
		)
		.await?;

		tracing::info!("Successfully executed {:?} request #{}", self.request_type, self.hash);

		Ok(())
	}
}

// private methods
impl Request {
	fn duration_to_parachain_blocks(duration: Duration) -> Result<u32, Error> {
		let num_blocks = duration.as_millis() / (runtime::MILLISECS_PER_BLOCK as u128);
		Ok(num_blocks.try_into()?)
	}

	fn calculate_deadline(
		opentime: u32,
		period: u32,
		payment_margin: Duration,
	) -> Result<Deadline, Error> {
		let margin_parachain_blocks = Self::duration_to_parachain_blocks(payment_margin)?;
		// if margin > period, we allow deadline to be before opentime. The rest of the code
		// can deal with the expired deadline as normal.
		let parachain_deadline = opentime
			.checked_add(period)
			.ok_or(Error::ArithmeticOverflow)?
			.checked_sub(margin_parachain_blocks)
			.ok_or(Error::ArithmeticUnderflow)?;

		Ok(Deadline { parachain: parachain_deadline })
	}

	/// Make a stellar transfer to fulfil the request
	#[tracing::instrument(
    name = "transfer_stellar_asset",
    skip(self, wallet),
    fields(
    request_type = ?self.request_type,
    request_id = ?self.hash,
    )
    )]
	async fn transfer_stellar_asset(
		&self,
		wallet: Arc<RwLock<StellarWallet>>,
	) -> Result<TransactionResponse, Error> {
		let destination_public_key = PublicKey::from_binary(self.stellar_address);
		let stroop_amount =
			primitives::BalanceConversion::lookup(self.amount).map_err(|_| Error::LookupError)?;
		let request_id = self.hash.0;

		let mut wallet = wallet.write().await;
		tracing::info!(
			"For {:?} request #{}: Sending {:?} stroops of {:?} to {:?} from {:?}",
			self.request_type,
			self.hash,
			stroop_amount,
			self.asset.clone(),
			destination_public_key,
			wallet,
		);

		let response = match self.request_type {
			RequestType::Redeem =>
				wallet
					.send_payment_to_address(
						destination_public_key.clone(),
						self.asset.clone(),
						stroop_amount,
						request_id,
						true,
					)
					.await,
			RequestType::Replace =>
				wallet
					.send_payment_to_address(
						destination_public_key.clone(),
						self.asset.clone(),
						stroop_amount,
						request_id,
						false,
					)
					.await,
		}
		.map_err(|e| Error::StellarWalletError(e))?;

		tracing::info!(
			"For {:?} request #{}: Successfully sent stellar payment to {:?} for {}",
			self.request_type,
			self.hash,
			destination_public_key,
			self.amount
		);
		Ok(response)
	}
}

pub struct PayAndExecute;
