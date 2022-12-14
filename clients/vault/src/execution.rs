use std::{collections::HashMap, convert::TryInto, sync::Arc, time::Duration};

use futures::{future::Either, try_join, StreamExt};
use governor::RateLimiter;
use sp_arithmetic::FixedPointNumber;
use sp_runtime::traits::StaticLookup;
use tokio::{
	sync::RwLock,
	time::{sleep, timeout},
};

use primitives::stellar::PublicKey;
use runtime::{
	types::FixedU128, CurrencyId, OraclePallet, RedeemPallet, RedeemRequestStatus, ReplacePallet,
	ReplaceRequestStatus, SecurityPallet, ShutdownSender, SpacewalkParachain,
	SpacewalkRedeemRequest, SpacewalkReplaceRequest, StellarPublicKeyRaw, StellarRelayPallet,
	UtilFuncs, VaultId, VaultRegistryPallet, H256,
};
use service::{spawn_cancelable, Error as ServiceError};
use stellar_relay_lib::sdk::{TransactionEnvelope, XdrCodec};
use wallet::StellarWallet;

use crate::{
	error::Error,
	oracle::{types::Slot, Proof, ProofExt, ProofStatus},
	system::VaultData,
	VaultIdManager, YIELD_RATE,
};

#[derive(Debug, Clone, PartialEq)]
struct Deadline {
	parachain: u32,
}

#[derive(Debug, Clone)]
pub struct Request {
	hash: H256,
	/// Deadline (unit: active block number) after which payments will no longer be attempted.
	deadline: Option<Deadline>,
	amount: u128,
	asset: CurrencyId,
	stellar_address: StellarPublicKeyRaw,
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

	/// Constructs a Request for the given InterBtcRedeemRequest
	pub fn from_redeem_request(
		hash: H256,
		request: SpacewalkRedeemRequest,
		payment_margin: Duration,
	) -> Result<Request, Error> {
		Ok(Request {
			hash,
			deadline: Some(Self::calculate_deadline(
				request.opentime,
				request.period,
				payment_margin,
			)?),
			amount: request.amount,
			asset: request.asset,
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
		Ok(Request {
			hash,
			deadline: Some(Self::calculate_deadline(
				request.accept_time,
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
		proof_ops: Arc<RwLock<dyn ProofExt>>,
	) -> Result<(), Error> {
		// ensure the deadline has not expired yet
		if let Some(ref deadline) = self.deadline {
			if parachain_rpc.get_current_active_block_number().await? >= deadline.parachain {
				return Err(Error::DeadlineExpired)
			}
		}

		let (tx_env, slot) = self.transfer_stellar_asset(vault.stellar_wallet).await?;

		let ops_read = proof_ops.read().await;
		// TODO refactor this once the improved 'OracleAgent' is implemented
		let proof: Proof = loop {
			let proof_status_result = ops_read.get_proof(slot as Slot).await;
			match proof_status_result {
				Ok(proof_status) => match proof_status {
					ProofStatus::Proof(p) => break p,
					ProofStatus::LackingEnvelopes => {},
					ProofStatus::NoEnvelopesFound => {},
					ProofStatus::NoTxSetFound => {},
					ProofStatus::WaitForTxSet => {},
				},
				Err(e) => {
					tracing::error!("Error while fetching proof: {:?}", e);
				},
			}
		};

		self.execute(parachain_rpc, tx_env, proof).await
	}

	/// Make a stellar transfer to fulfil the request
	#[tracing::instrument(
	name = "transfer_stellar_asset",
	skip(self),
	fields(
	request_type = ?self.request_type,
	request_id = ?self.hash,
	)
	)]
	async fn transfer_stellar_asset(
		&self,
		wallet: Arc<StellarWallet>,
	) -> Result<(TransactionEnvelope, u32), Error> {
		let destination_public_key = PublicKey::from_binary(self.stellar_address);
		let stellar_asset =
			primitives::AssetConversion::lookup(self.asset).map_err(|_| Error::LookupError)?;
		let stroop_amount = self.amount as i64;
		let memo_hash = self.hash.0;

		let result = wallet
			.send_payment_to_address(
				destination_public_key.clone(),
				stellar_asset,
				stroop_amount,
				memo_hash,
			)
			.await;

		match result {
			Ok((response, tx_env)) => {
				let slot = response.ledger;
				tracing::info!(
					"Successfully sent stellar payment to {:?} for {}",
					destination_public_key,
					self.amount
				);
				Ok((tx_env, slot))
			},
			Err(e) => Err(Error::StellarWalletError(e)),
		}
	}

	/// Executes the request. Upon failure it will retry
	async fn execute<P: ReplacePallet + RedeemPallet>(
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
				match result {
					Ok(ok) => Ok(ok),
					Err(err) if err.is_rpc_disconnect_error() =>
						Err(runtime::RetryPolicy::Throw(err)),
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
	wallet: Arc<StellarWallet>,
	proof_ops: Arc<RwLock<dyn ProofExt>>,
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

	// Check if some of the requests that are open already have a corresponding payment on Stellar
	// and are just waiting to be executed on the parachain

	// Query the latest 200 transactions for the targeted vault account and check if any of
	// them is targeted
	let transactions_result = wallet.get_latest_transactions(0, 200, false);
	match transactions_result {
		Ok(transactions) => {
			tracing::info!("Checking {} transactions for payments", transactions.len());
			for transaction in transactions {
				if rate_limiter.check().is_ok() {
					// give the outer `select` a chance to check the shutdown signal
					tokio::task::yield_now().await;
				}

				if let Some(request) = get_request_for_stellar_tx(&transaction, &open_requests) {
					// remove request from the hashmap
					open_requests.retain(|&key, _| key != request.hash);

					tracing::info!(
						"{:?} request #{:?} has valid Stellar payment - processing...",
						request.request_type,
						request.hash
					);

					// start a new task to execute on the parachain and make copies of the
					// variables we move into the task
					let parachain_rpc = parachain_rpc.clone();
					let vault_id_manager = vault_id_manager.clone();
					spawn_cancelable(shutdown_tx.subscribe(), async move {
						let tx_env = TransactionEnvelope::from_xdr(&transaction.envelope_xdr);
						match tx_env {
							Ok(tx_env) => {
								let slot = transaction.ledger as Slot;

								// Loop pending proofs until it is ready
								// TODO refactor this once improved 'OracleAgent' is ready
								let mut proof: Option<Proof> = None;
								timeout(Duration::from_secs(60), async {
									loop {
										let ops_read = proof_ops.read().await;
										let proof_status = ops_read
											.get_proof(slot)
											.await
											.expect("Failed to get proof");

										match proof_status {
											ProofStatus::Proof(p) => {
												proof = Some(p);
												break
											},
											ProofStatus::LackingEnvelopes => {},
											ProofStatus::NoEnvelopesFound => {},
											ProofStatus::NoTxSetFound => {},
											ProofStatus::WaitForTxSet => {},
										}

										// Wait a bit before trying again
										sleep(Duration::from_secs(3)).await;
									}
								})
								.await;

								if let Some(proof) = proof {
									if let Err(e) =
										request.execute(parachain_rpc.clone(), tx_env, proof).await
									{
										tracing::error!(
											"Failed to execute request #{}: {}",
											request.hash,
											e
										);
									}
								}
							},
							Err(error) => {
								tracing::error!("Failed to decode transaction envelope");
							},
						}
					});
				}
			}
		},
		Err(error) => {
			tracing::error!("Failed to get transactions from Stellar: {}", error);
		},
	}

	// All requests remaining in the hashmap did not have a Stellar payment yet, so pay
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
/// on the amount of assets that is transferred to the address.
fn get_request_for_stellar_tx(
	tx: &TransactionResponse,
	hash_map: &HashMap<H256, Request>,
) -> Option<Request> {
	let hash = tx.get_op_return()?;
	let request = hash_map.get(&hash)?;
	let paid_amount = tx.get_payment_amount_to(request.btc_address.to_payload().ok()?)?;
	if paid_amount as u128 >= request.amount {
		Some(request.clone())
	} else {
		None
	}
}
