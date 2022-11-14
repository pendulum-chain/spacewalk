use std::{collections::HashMap, convert::TryInto, time::Duration};

use futures::{future::Either, stream::StreamExt, try_join, TryStreamExt};
use governor::RateLimiter;
use tokio::time::sleep;
use tokio_stream::wrappers::BroadcastStream;

use runtime::{
	CurrencyId, Error as RuntimeError, FixedPointNumber, FixedU128, OraclePallet, PrettyPrint,
	SpacewalkParachain, StellarPublicKey, UtilFuncs, VaultId, VaultRegistryPallet, H256,
};
use service::{spawn_cancelable, DynBitcoinCoreApi, Error as ServiceError, ShutdownSender};

use crate::{
	error::Error,
	metrics::update_bitcoin_metrics,
	stellar_wallet::StellarWallet,
	system::{VaultData, VaultIdManager},
	VaultIdManager, YIELD_RATE,
};

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

		let bitcoin_deadline = btc_start_height
			.checked_add(parachain_blocks_to_bitcoin_blocks_rounded_up(period)?)
			.ok_or(Error::ArithmeticOverflow)?
			.checked_sub(parachain_blocks_to_bitcoin_blocks_rounded_up(margin_parachain_blocks)?)
			.ok_or(Error::ArithmeticUnderflow)?;

		Ok(Deadline { bitcoin: bitcoin_deadline, parachain: parachain_deadline })
	}

	/// Constructs a Request for the given InterBtcRedeemRequest
	pub fn from_redeem_request(
		hash: H256,
		request: InterBtcRedeemRequest,
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
			amount: request.amount_btc,
			asset: request.asset,
			stellar_address: request.btc_address,
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
	) -> Result<(), Error> {
		// ensure the deadline has not expired yet
		if let Some(ref deadline) = self.deadline {
			if parachain_rpc.get_current_active_block_number().await? >= deadline.parachain &&
				vault.stellar_wallet.get_block_count().await? >= deadline.bitcoin as u64
			{
				return Err(Error::DeadlineExpired)
			}
		}

		let tx_metadata = self
			.transfer_btc(&parachain_rpc, &vault.stellar_wallet, self.vault_id.clone())
			.await?;
		let _ = update_bitcoin_metrics(&vault, tx_metadata.fee, self.fee_budget).await;
		self.execute(parachain_rpc, tx_metadata).await
	}

	/// Make a bitcoin transfer to fulfil the request
	#[tracing::instrument(
    name = "transfer_btc",
    skip(self, parachain_rpc, stellar_wallet),
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
		stellar_wallet: &StellarWallet,
		vault_id: VaultId,
	) -> Result<TransactionMetadata, Error> {
		let fee_rate = self.get_fee_rate(parachain_rpc).await?;

		tracing::debug!("Using fee_rate = {} sat/vByte", fee_rate.0);

		let txid = stellar_wallet
			.create_and_send_transaction(
				self.btc_address
					.to_address(stellar_wallet.network())
					.map_err(BitcoinError::ConversionError)?,
				self.amount as u64,
				fee_rate,
				Some(self.hash),
			)
			.await?;
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
/// bitcoin blockchain to see if a payment has already been made.
#[allow(clippy::too_many_arguments)]
pub async fn execute_open_requests(
	shutdown_tx: ShutdownSender,
	parachain_rpc: SpacewalkParachain,
	vault_id_manager: VaultIdManager,
	read_only_stellar_wallet: DynBitcoinCoreApi,
	num_confirmations: u32,
	payment_margin: Duration,
) -> Result<(), ServiceError<Error>> {
	let parachain_rpc = &parachain_rpc;
	let vault_id = parachain_rpc.get_account_id().clone();

	// get all redeem and replace requests
	let (redeem_requests, replace_requests) = try_join!(
		parachain_rpc.get_vault_redeem_requests(vault_id.clone()),
		parachain_rpc.get_old_vault_replace_requests(vault_id.clone()),
	)?;

	let open_redeems = redeem_requests
		.into_iter()
		.filter(|(_, request)| request.status == RedeemRequestStatus::Pending)
		.filter_map(|(hash, request)| {
			Request::from_redeem_request(hash, request, payment_margin).ok()
		});

	let open_replaces = replace_requests
		.into_iter()
		.filter(|(_, request)| request.status == ReplaceRequestStatus::Pending)
		.filter_map(|(hash, request)| {
			Request::from_replace_request(hash, request, payment_margin).ok()
		});

	// collect all requests into a hashmap, indexed by their id
	let mut open_requests = open_redeems
		.chain(open_replaces)
		.map(|x| (x.hash, x))
		.collect::<HashMap<_, _>>();

	let rate_limiter = RateLimiter::direct(YIELD_RATE);

	// iterate through transactions in reverse order, starting from those in the mempool, and
	// gracefully fail on encountering a pruned blockchain
	let mut transaction_stream =
		bitcoin::reverse_stream_transactions(&read_only_stellar_wallet, btc_start_height).await?;
	while let Some(result) = transaction_stream.next().await {
		if rate_limiter.check().is_ok() {
			// give the outer `select` a chance to check the shutdown signal
			tokio::task::yield_now().await;
		}

		let tx = match result {
			Ok(x) => x,
			Err(e) => {
				tracing::warn!("Failed to process transaction: {}", e);
				continue
			},
		};

		// get the request this transaction corresponds to, if any
		if let Some(request) = get_request_for_btc_tx(&tx, &open_requests) {
			// remove request from the hashmap
			open_requests.retain(|&key, _| key != request.hash);

			tracing::info!(
				"{:?} request #{:?} has valid bitcoin payment - processing...",
				request.request_type,
				request.hash
			);

			// start a new task to (potentially) await confirmation and to execute on the parachain
			// make copies of the variables we move into the task
			let parachain_rpc = parachain_rpc.clone();
			let stellar_wallet = vault_id_manager.clone();
			spawn_cancelable(shutdown_tx.subscribe(), async move {
				let stellar_wallet =
					match stellar_wallet.get_stellar_wallet(&request.vault_id).await {
						Some(x) => x,
						None => {
							tracing::error!(
								"Failed to fetch bitcoin rpc for vault {}",
								request.vault_id.pretty_print()
							);
							return // nothing we can do - bail
						},
					};

				match request
					.wait_for_inclusion(
						&parachain_rpc,
						&stellar_wallet,
						num_confirmations,
						tx.txid(),
						auto_rbf,
					)
					.await
				{
					Ok(tx_metadata) => {
						if let Err(e) = request.execute(parachain_rpc.clone(), tx_metadata).await {
							tracing::error!("Failed to execute request #{}: {}", request.hash, e);
						}
					},
					Err(e) => {
						tracing::error!(
							"Error while waiting for inclusion for request #{}: {}",
							request.hash,
							e
						);
					},
				}
			});
		}
	}

	// All requests remaining in the hashmap did not have a bitcoin payment yet, so pay
	// and execute all of these
	for (_, request) in open_requests {
		// there are potentially a large number of open requests - pay and execute each
		// in a separate task to ensure that awaiting confirmations does not significantly
		// delay other requests
		// make copies of the variables we move into the task
		let parachain_rpc = parachain_rpc.clone();
		let vault_id_manager = vault_id_manager.clone();
		spawn_cancelable(shutdown_tx.subscribe(), async move {
			let vault = match vault_id_manager.get_vault(&request.vault_id).await {
				Some(x) => x,
				None => {
					tracing::error!(
						"Failed to fetch bitcoin rpc for vault {}",
						request.vault_id.pretty_print()
					);
					return // nothing we can do - bail
				},
			};

			tracing::info!(
				"{:?} request #{:?} found without bitcoin payment - processing...",
				request.request_type,
				request.hash
			);

			match request.pay_and_execute(parachain_rpc, vault, num_confirmations, auto_rbf).await {
				Ok(_) => tracing::info!(
					"{:?} request #{:?} successfully executed",
					request.request_type,
					request.hash
				),
				Err(e) => tracing::info!(
					"{:?} request #{:?} failed to process: {}",
					request.request_type,
					request.hash,
					e
				),
			}
		});
	}

	Ok(())
}

/// Get the Request from the hashmap that the given Transaction satisfies, based
/// on the OP_RETURN and the amount of btc that is transfered to the address
fn get_request_for_btc_tx(tx: &Transaction, hash_map: &HashMap<H256, Request>) -> Option<Request> {
	let hash = tx.get_op_return()?;
	let request = hash_map.get(&hash)?;
	let paid_amount = tx.get_payment_amount_to(request.btc_address.to_payload().ok()?)?;
	if paid_amount as u128 >= request.amount {
		Some(request.clone())
	} else {
		None
	}
}
