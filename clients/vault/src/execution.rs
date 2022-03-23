use crate::{
    error::Error,
    horizon::{fetch_latest_txs, process_new_transaction, HorizonTransactionsResponse},
    VaultIdManager,
};
use bitcoin::{
    BitcoinCoreApi, PartialAddress, Transaction, TransactionExt, TransactionMetadata,
    BLOCK_INTERVAL as BITCOIN_BLOCK_INTERVAL,
};
use futures::{
    stream::{self, StreamExt},
    try_join, TryStreamExt,
};
use runtime::{
    BtcAddress, BtcRelayPallet, H256Le, InterBtcParachain, InterBtcRedeemRequest, InterBtcRefundRequest,
    InterBtcReplaceRequest, IssuePallet, RedeemPallet, RedeemRequestStatus, RefundPallet, ReplacePallet,
    ReplaceRequestStatus, RequestRefundEvent, SecurityPallet, UtilFuncs, VaultId, VaultRegistryPallet, H256,
};
use service::{spawn_cancelable, ShutdownSender};
use std::{collections::HashMap, convert::TryInto, time::Duration};
use tokio::time::sleep;

const ON_FORK_RETRY_DELAY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq)]
struct Deadline {
    parachain: u32,
    bitcoin: u32,
}

#[derive(Debug, Clone)]
pub struct Request {
    hash: H256,
    btc_height: Option<u32>,
    /// Deadline (unit: active block number) after which payments will no longer be attempted.
    deadline: Option<Deadline>,
    amount: u128,
    btc_address: BtcAddress,
    request_type: RequestType,
    vault_id: VaultId,
    fee_budget: Option<u128>,
}

pub fn parachain_blocks_to_bitcoin_blocks_rounded_up(parachain_blocks: u32) -> Result<u32, Error> {
    let millis = (parachain_blocks as u64)
        .checked_mul(runtime::MILLISECS_PER_BLOCK)
        .ok_or(Error::ArithmeticOverflow)?;

    let denominator = BITCOIN_BLOCK_INTERVAL.as_millis();

    // do -num_bitcoin_blocks = ceil(millis / demoninator)
    let num_bitcoin_blocks = (millis as u128)
        .checked_add(denominator)
        .ok_or(Error::ArithmeticOverflow)?
        .checked_sub(1)
        .ok_or(Error::ArithmeticUnderflow)?
        .checked_div(denominator)
        .ok_or(Error::ArithmeticUnderflow)?;

    Ok(num_bitcoin_blocks.try_into()?)
}

#[derive(Debug, Copy, Clone)]
pub enum RequestType {
    Redeem,
    Replace,
    Refund,
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

        Ok(Deadline {
            bitcoin: bitcoin_deadline,
            parachain: parachain_deadline,
        })
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
            btc_height: Some(request.btc_height),
            amount: request.amount_btc,
            btc_address: request.btc_address,
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
            btc_height: Some(request.btc_height),
            amount: request.amount,
            btc_address: request.btc_address,
            request_type: RequestType::Replace,
            vault_id: request.old_vault,
            fee_budget: None,
        })
    }

    /// Constructs a Request for the given InterBtcRefundRequest
    fn from_refund_request(hash: H256, request: InterBtcRefundRequest, btc_height: u32) -> Request {
        Request {
            hash,
            deadline: None,
            btc_height: Some(btc_height),
            amount: request.amount_btc,
            btc_address: request.btc_address,
            request_type: RequestType::Refund,
            vault_id: request.vault,
            fee_budget: Some(request.transfer_fee_btc),
        }
    }

    /// Constructs a Request for the given RequestRefundEvent
    pub fn from_refund_request_event(request: &RequestRefundEvent) -> Request {
        Request {
            btc_address: request.btc_address,
            amount: request.amount,
            hash: request.refund_id,
            btc_height: None,
            deadline: None,
            request_type: RequestType::Refund,
            vault_id: request.vault_id.clone(),
            fee_budget: None,
        }
    }

    /// Makes the bitcoin transfer and executes the request
    pub async fn pay_and_execute<
        B: BitcoinCoreApi + Clone,
        P: ReplacePallet
            + RefundPallet
            + BtcRelayPallet
            + RedeemPallet
            + SecurityPallet
            + VaultRegistryPallet
            + UtilFuncs
            + Clone
            + Send
            + Sync,
    >(
        &self,
        parachain_rpc: P,
        btc_rpc: B,
        num_confirmations: u32,
    ) -> Result<(), Error> {
        // ensure the deadline has not expired yet
        if let Some(ref deadline) = self.deadline {
            if parachain_rpc.get_current_active_block_number().await? >= deadline.parachain
                && btc_rpc.get_block_count().await? >= deadline.bitcoin as u64
            {
                return Err(Error::DeadlineExpired);
            }
        }

        let tx_metadata = self
            .transfer_btc(&parachain_rpc, btc_rpc, num_confirmations, self.vault_id.clone())
            .await?;
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
        B: BitcoinCoreApi + Clone,
        P: BtcRelayPallet + VaultRegistryPallet + UtilFuncs + Clone + Send + Sync,
    >(
        &self,
        parachain_rpc: &P,
        btc_rpc: B,
        num_confirmations: u32,
        vault_id: VaultId,
    ) -> Result<TransactionMetadata, Error> {
        let tx = btc_rpc
            .create_transaction(self.btc_address, self.amount as u64, Some(self.hash))
            .await?;
        let recipient = tx.recipient.clone();
        tracing::info!("Sending bitcoin to {}", recipient);

        let return_to_self_addresses = tx
            .transaction
            .extract_output_addresses()
            .into_iter()
            .filter(|x| x != &self.btc_address)
            .collect::<Vec<_>>();

        // register return-to-self address if it exists
        match return_to_self_addresses.as_slice() {
            [] => {} // no return-to-self
            [address] => {
                // one return-to-self address, make sure it is registered
                let wallet = parachain_rpc.get_vault(&vault_id).await?.wallet;
                if !wallet.addresses.contains(address) {
                    tracing::info!(
                        "Registering address {:?}",
                        address
                            .encode_str(btc_rpc.network())
                            .unwrap_or(format!("{:?}", address)),
                    );
                    parachain_rpc.register_address(&vault_id, *address).await?;
                }
            }
            _ => return Err(Error::TooManyReturnToSelfAddresses),
        };

        let txid = btc_rpc.send_transaction(tx).await?;

        loop {
            let tx_metadata = btc_rpc.wait_for_transaction_metadata(txid, num_confirmations).await?;

            tracing::info!("Awaiting parachain confirmations...");

            match parachain_rpc
                .wait_for_block_in_relay(
                    H256Le::from_bytes_le(&tx_metadata.block_hash.to_vec()),
                    Some(num_confirmations),
                )
                .await
            {
                Ok(_) => {
                    tracing::info!("Bitcoin successfully sent and relayed");
                    return Ok(tx_metadata);
                }
                Err(e) if e.is_invalid_chain_id() => {
                    // small delay to prevent spamming
                    sleep(ON_FORK_RETRY_DELAY).await;
                    // re-fetch the metadata - it might be in a different block now
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Executes the request. Upon failure it will retry
    async fn execute<P: ReplacePallet + RedeemPallet + RefundPallet>(
        &self,
        parachain_rpc: P,
        tx_metadata: TransactionMetadata,
    ) -> Result<(), Error> {
        // select the execute function based on request_type
        let execute = match self.request_type {
            RequestType::Redeem => RedeemPallet::execute_redeem,
            RequestType::Replace => ReplacePallet::execute_replace,
            RequestType::Refund => RefundPallet::execute_refund,
        };

        match (self.fee_budget, tx_metadata.fee.map(|x| x.abs().as_sat() as u128)) {
            (Some(budget), Some(actual)) if budget < actual => {
                tracing::warn!(
                    "Spent more on bitcoin inclusion fee than budgeted: spent {} satoshi; budget was {}",
                    actual,
                    budget
                );
            }
            _ => {}
        }

        // Retry until success or timeout, explicitly handle the cases
        // where the redeem has expired or the rpc has disconnected
        runtime::notify_retry(
            || (execute)(&parachain_rpc, self.hash, &tx_metadata.proof, &tx_metadata.raw_tx),
            |result| async {
                match result {
                    Ok(ok) => Ok(ok),
                    Err(err) if err.is_commit_period_expired() => Err(runtime::RetryPolicy::Throw(err)),
                    Err(err) if err.is_rpc_disconnect_error() => Err(runtime::RetryPolicy::Throw(err)),
                    Err(err) if err.is_invalid_chain_id() => Err(runtime::RetryPolicy::Throw(err)),
                    Err(err) => Err(runtime::RetryPolicy::Skip(err)),
                }
            },
        )
        .await?;

        Ok(())
    }
}

/// Queries the parachain for open requests and executes them. It checks the
/// bitcoin blockchain to see if a payment has already been made.
pub async fn execute_open_requests(
    shutdown_tx: ShutdownSender,
    parachain_rpc: InterBtcParachain,
    num_confirmations: u32,
    payment_margin: Duration,
    process_refunds: bool,
) -> Result<(), Error> {
    tracing::info!("In execute_open_requests");

    let parachain_rpc = &parachain_rpc;
    let vault_id = parachain_rpc.get_account_id().clone();

    // get all redeem, replace and refund requests
    let (redeem_requests, replace_requests, refund_requests) = try_join!(
        parachain_rpc.get_vault_redeem_requests(vault_id.clone()),
        parachain_rpc.get_old_vault_replace_requests(vault_id.clone()),
        async {
            if process_refunds {
                // for refunds, we use the btc_height of the issue
                let refunds = parachain_rpc.get_vault_refund_requests(vault_id).await?;
                stream::iter(refunds)
                    .then(|(refund_id, refund)| async move {
                        Ok((
                            refund_id,
                            refund.clone(),
                            parachain_rpc.get_issue_request(refund.issue_id).await?.btc_height,
                        ))
                    })
                    .try_collect()
                    .await
            } else {
                Ok(Vec::new())
            }
        },
    )?;

    let res = fetch_latest_txs().await;
    let transactions = match res {
        Ok(txs) => txs._embedded.records,
        Err(e) => {
            tracing::warn!("Failed to fetch transactions: {:?}", e);
            return Ok(());
        }
    };

    Ok(())
}
