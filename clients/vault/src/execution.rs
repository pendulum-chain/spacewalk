use std::{collections::HashMap, convert::TryInto, sync::Arc, time::Duration};

use futures::{future::Either, stream::StreamExt, try_join, TryStreamExt};
use tokio::time::sleep;
use tokio_stream::wrappers::BroadcastStream;

use runtime::{
	CurrencyId, Error as RuntimeError, FixedPointNumber, FixedU128, H256Le, OraclePallet,
	PartialAddress, RedeemPallet, RedeemRequestStatus, ReplacePallet, ReplaceRequestStatus,
	SecurityPallet, SpacewalkParachain, SpacewalkRedeemRequest, SpacewalkReplaceRequest,
	StellarPublicKey, StellarRelayPallet, UtilFuncs, VaultId, VaultRegistryPallet, H256,
};
use service::{spawn_cancelable, Error as ServiceError, ShutdownSender};
use wallet::StellarWallet;

use crate::{error::Error, system::VaultData, VaultIdManager};

#[derive(Debug, Clone, PartialEq)]
struct Deadline {
	parachain: u32,
	bitcoin: u32,
}

#[derive(Debug, Clone)]
pub struct Request {
	hash: H256,
	/// Deadline (unit: active block number) after which payments will no longer be attempted.
	deadline: Option<Deadline>,
	amount: u128,
	asset: CurrencyId,
	stellar_address: StellarPublicKey,
	request_type: RequestType,
	vault_id: VaultId,
	fee_budget: Option<u128>,
}

pub fn parachain_blocks_to_stellar_blocks_rounded_up(parachain_blocks: u32) -> Result<u32, Error> {
	let millis = (parachain_blocks as u64)
		.checked_mul(runtime::MILLISECS_PER_BLOCK)
		.ok_or(Error::ArithmeticOverflow)?;

	let denominator = stellar_relay::BLOCK_INTERVAL.as_millis();

	let num_stellar_blocks = (millis as u128)
		.checked_add(denominator)
		.ok_or(Error::ArithmeticOverflow)?
		.checked_sub(1)
		.ok_or(Error::ArithmeticUnderflow)?
		.checked_div(denominator)
		.ok_or(Error::ArithmeticUnderflow)?;

	Ok(num_stellar_blocks.try_into()?)
}

#[derive(Debug, Copy, Clone)]
pub enum RequestType {
	Redeem,
	Replace,
}

impl Request {
	fn duration_to_parachain_blocks(duration: Duration) -> Result<u32, Error> {
		let num_blocks = duration.as_millis() / (runtime::MILLISECS_PER_BLOCK as u128);
		Ok(num_blocks.try_into()?)
	}

	fn calculate_deadline(
		opentime: u32,
		btc_start_height: u32,
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

		let stellar_deadline = btc_start_height
			.checked_add(parachain_blocks_to_stellar_blocks_rounded_up(period)?)
			.ok_or(Error::ArithmeticOverflow)?
			.checked_sub(parachain_blocks_to_stellar_blocks_rounded_up(margin_parachain_blocks)?)
			.ok_or(Error::ArithmeticUnderflow)?;

		Ok(Deadline { bitcoin: stellar_deadline, parachain: parachain_deadline })
	}

	/// Constructs a Request for the given SpacewalkRedeemRequest
	pub fn from_redeem_request(
		hash: H256,
		request: SpacewalkRedeemRequest,
		payment_margin: Duration,
	) -> Result<Request, Error> {
		Ok(Request {
			hash,
			deadline: Some(Self::calculate_deadline(
				request.opentime,
				request.btc_height,
				request.period,
				payment_margin,
			)?),
			amount: request.amount,
			asset: request.asset,
			stellar_address: request.stellar_address,
			request_type: RequestType::Redeem,
			vault_id: request.vault,
			fee_budget: Some(request.transfer_fee_btc),
		})
	}

	/// Constructs a Request for the given InterBtcReplaceRequest
	pub fn from_replace_request(
		hash: H256,
		request: InterBtcReplaceRequest,
		payment_margin: Duration,
	) -> Result<Request, Error> {
		Ok(Request {
			hash,
			deadline: Some(Self::calculate_deadline(
				request.accept_time,
				request.btc_height,
				request.period,
				payment_margin,
			)?),
			amount: request.amount,
			asset: request.asset,
			stellar_address: request.stellar_address,
			request_type: RequestType::Replace,
			vault_id: request.old_vault,
			fee_budget: None,
		})
	}

	/// returns the fee rate in sat/vByte
	async fn get_fee_rate<P: OraclePallet + Send + Sync>(
		&self,
		parachain_rpc: &P,
	) -> Result<SatPerVbyte, Error> {
		let fee_rate: FixedU128 = parachain_rpc.get_bitcoin_fees().await?;
		let rate = fee_rate
			.into_inner()
			.checked_div(FixedU128::accuracy())
			.ok_or(Error::ArithmeticUnderflow)?
			.try_into()?;
		Ok(SatPerVbyte(rate))
	}

	/// Makes the bitcoin transfer and executes the request
	pub async fn pay_and_execute<
		P: ReplacePallet
			+ BtcRelayPallet
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
		num_confirmations: u32,
		auto_rbf: bool,
	) -> Result<(), Error> {
		// ensure the deadline has not expired yet
		if let Some(ref deadline) = self.deadline {
			if parachain_rpc.get_current_active_block_number().await? >= deadline.parachain &&
				vault.btc_rpc.get_block_count().await? >= deadline.bitcoin as u64
			{
				return Err(Error::DeadlineExpired)
			}
		}

		let tx_metadata = self
			.transfer_btc(
				&parachain_rpc,
				&vault.btc_rpc,
				num_confirmations,
				self.vault_id.clone(),
				auto_rbf,
			)
			.await?;
		let _ = update_bitcoin_metrics(&vault, tx_metadata.fee, self.fee_budget).await;
		self.execute(parachain_rpc, tx_metadata).await
	}

	/// Make a bitcoin transfer to fulfil the request
	#[tracing::instrument(
			name = "transfer_btc",
			skip(self, parachain_rpc, btc_rpc),
			fields(
					request_type = ?self.request_type,
					request_id = ?self.hash,
			)
	)]
	async fn transfer_btc<
		P: OraclePallet + BtcRelayPallet + VaultRegistryPallet + UtilFuncs + Clone + Send + Sync,
	>(
		&self,
		parachain_rpc: &P,
		btc_rpc: &DynBitcoinCoreApi,
		num_confirmations: u32,
		vault_id: VaultId,
		auto_rbf: bool,
	) -> Result<TransactionMetadata, Error> {
		let fee_rate = self.get_fee_rate(parachain_rpc).await?;

		tracing::debug!("Using fee_rate = {} sat/vByte", fee_rate.0);

		let txid = btc_rpc
			.create_and_send_transaction(
				self.btc_address
					.to_address(btc_rpc.network())
					.map_err(BitcoinError::ConversionError)?,
				self.amount as u64,
				fee_rate,
				Some(self.hash),
			)
			.await?;

		self.wait_for_inclusion(parachain_rpc, btc_rpc, num_confirmations, txid, auto_rbf)
			.await
	}

	#[tracing::instrument(
			name = "wait_for_inclusion",
			skip(self, parachain_rpc, btc_rpc),
			fields(
					request_type = ?self.request_type,
					request_id = ?self.hash,
			)
	)]
	async fn wait_for_inclusion<
		P: OraclePallet + BtcRelayPallet + VaultRegistryPallet + UtilFuncs + Clone + Send + Sync,
	>(
		&self,
		parachain_rpc: &P,
		btc_rpc: &DynBitcoinCoreApi,
		num_confirmations: u32,
		mut txid: Txid,
		auto_rbf: bool,
	) -> Result<TransactionMetadata, Error> {
		'outer: loop {
			tracing::info!("Awaiting bitcoin confirmations for {txid}");

			let txid_copy = txid; // we get borrow check error if we don't use a copy

			let fee_rate_subscription = parachain_rpc.on_fee_rate_change();
			let fee_rate_subscription = BroadcastStream::new(fee_rate_subscription);
			let subscription = fee_rate_subscription
				.map_err(Into::<Error>::into)
				.and_then(|x| {
					tracing::debug!("Received new inclusion fee estimate {}...", x);

					let ret: Result<SatPerVbyte, _> = x
						.into_inner()
						.checked_div(FixedU128::accuracy())
						.ok_or(Error::ArithmeticUnderflow)
						.and_then(|x| x.try_into().map(SatPerVbyte).map_err(Into::<Error>::into));
					futures::future::ready(ret)
				})
				.filter(|_| futures::future::ready(auto_rbf)) // if auto-rbf is disabled, don't propagate the events
				.try_filter_map(|x| async move {
					match btc_rpc.fee_rate(txid).await {
						Ok(current_fee) =>
							if x > current_fee {
								Ok(Some((current_fee, x)))
							} else {
								Ok(None)
							},
						Err(e) => {
							tracing::debug!("Failed to get fee_rate: {}", e);
							Ok(None)
						},
					}
				})
				.filter_map(|x| async {
					match btc_rpc.is_in_mempool(txid_copy).await {
						Ok(false) => {
							//   if not in mempool anymore, don't propagate the event (even if it is
							// an error)
							tracing::debug!("Txid not in mempool anymore...");
							None
						},
						Ok(true) => {
							tracing::debug!("Txid is still in mempool...");
							Some(x)
						},
						Err(e) => {
							tracing::warn!("Unexpected bitcoin error: {}", e);
							Some(Err(e.into()))
						},
					}
				});

			let wait_for_transaction_metadata =
				btc_rpc.wait_for_transaction_metadata(txid, num_confirmations);
			futures::pin_mut!(subscription);

			let mut metadata_fut = wait_for_transaction_metadata;

			// The code below looks a little bit complicated but the idea is simple:
			// we keep waiting for inclusion until it's either included in the bitcoin chain,
			// or we successfully bump fees
			let tx_metadata =
				loop {
					match futures::future::select(metadata_fut, subscription.next()).await {
						Either::Left((result, _)) => break result?,
						Either::Right((None, _)) =>
							return Err(Error::RuntimeError(RuntimeError::ChannelClosed)),
						Either::Right((Some(Err(x)), continuation)) => {
							tracing::warn!("Received an error from the fee rate subscription: {x}");
							// continue with the unchanged fee rate
							metadata_fut = continuation;
						},
						Either::Right((Some(Ok((old_fee, new_fee))), continuation)) => {
							tracing::debug!(
								"Attempting to bump fee rate from {} to {}...",
								old_fee.0,
								new_fee.0
							);
							match btc_rpc
								.bump_fee(
									&txid,
									self.btc_address
										.to_address(btc_rpc.network())
										.map_err(BitcoinError::ConversionError)?,
									new_fee,
								)
								.await
							{
								Ok(new_txid) => {
									tracing::info!(
										"Bumped fee rate. Old txid = {txid}, new txid = {new_txid}"
									);
									txid = new_txid;
									continue 'outer
								},
								Err(x) if x.rejected_by_network_rules() => {
									// bump not big enough. This is not unexpected, so only debug
									// print
									tracing::debug!("Failed to bump fees: {:?}", x);
								},
								Err(x) if x.could_be_insufficient_funds() => {
									// Unexpected: likely (but no certainly) there are insufficient
									// funds in the wallet to pay the increased fee.
									tracing::warn!("Failed to bump fees - likely due to insufficient funds: {:?}", x);
								},
								Err(x) => {
									// unexpected error. Just continue waiting for the original tx
									tracing::warn!(
										"Failed to bump fees due to unexpected reasons: {:?}",
										x
									);
								},
							};
							metadata_fut = continuation;
						},
					}
				};

			tracing::info!("Awaiting parachain confirmations...");

			match parachain_rpc
				.wait_for_block_in_relay(
					H256Le::from_bytes_le(&tx_metadata.block_hash),
					Some(num_confirmations),
				)
				.await
			{
				Ok(_) => {
					tracing::info!("Bitcoin successfully sent and relayed");
					return Ok(tx_metadata)
				},
				Err(e) if e.is_invalid_chain_id() => {
					// small delay to prevent spamming
					sleep(ON_FORK_RETRY_DELAY).await;
					// re-fetch the metadata - it might be in a different block now
					continue
				},
				Err(e) => return Err(e.into()),
			}
		}
	}

	/// Executes the request. Upon failure it will retry
	async fn execute<P: ReplacePallet + RedeemPallet>(
		&self,
		parachain_rpc: P,
		tx_metadata: TransactionMetadata,
	) -> Result<(), Error> {
		// select the execute function based on request_type
		let execute = match self.request_type {
			RequestType::Redeem => RedeemPallet::execute_redeem,
			RequestType::Replace => ReplacePallet::execute_replace,
		};

		match (self.fee_budget, tx_metadata.fee.map(|x| x.abs().to_sat() as u128)) {
			(Some(budget), Some(actual)) if budget < actual => {
				tracing::warn!(
									"Spent more on bitcoin inclusion fee than budgeted: spent {} satoshi; budget was {}",
									actual,
									budget
							);
			},
			_ => {},
		}

		// Retry until success or timeout, explicitly handle the cases
		// where the redeem has expired or the rpc has disconnected
		runtime::notify_retry(
			|| (execute)(&parachain_rpc, self.hash, &tx_metadata.proof, &tx_metadata.raw_tx),
			|result| async {
				match result {
					Ok(ok) => Ok(ok),
					Err(err) if err.is_rpc_disconnect_error() =>
						Err(runtime::RetryPolicy::Throw(err)),
					Err(err) if err.is_invalid_chain_id() => Err(runtime::RetryPolicy::Throw(err)),
					Err(err) => Err(runtime::RetryPolicy::Skip(err)),
				}
			},
		)
		.await?;

		tracing::info!("Executed request #{:?}", self.hash);

		Ok(())
	}
}

/// Queries the parachain for open requests and executes them. It checks the
/// stellar blockchain to see if a payment has already been made.
#[allow(clippy::too_many_arguments)]
pub async fn execute_open_requests(
	shutdown_tx: ShutdownSender,
	parachain_rpc: SpacewalkParachain,
	vault_id_manager: VaultIdManager,
	read_only_stellar_wallet: Arc<StellarWallet>,
	payment_margin: Duration,
) -> Result<(), ServiceError<Error>> {
	// TODO
	Ok(())
}
