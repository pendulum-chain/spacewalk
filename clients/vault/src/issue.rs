use std::{
	collections::HashMap, convert::TryFrom, fmt::Debug, num::TryFromIntError,
	string::FromUtf8Error, sync::Arc,
};

use async_trait::async_trait;
use itertools::Itertools;
use sp_runtime::traits::{LookupError, StaticLookup};
use tokio::sync::RwLock;

use primitives::{stellar::Memo, CurrencyId};
use runtime::{
	CancelIssueEvent, ExecuteIssueEvent, IssueId, IssuePallet, IssueRequestsMap, RequestIssueEvent,
	SpacewalkIssueRequest, SpacewalkParachain, StaticEvent, H256,
};
use service::Error as ServiceError;
use stellar_relay_lib::sdk::{
	types::{Int64, PaymentOp, TransactionV1Envelope},
	Asset, PublicKey, SecretKey, Transaction, TransactionEnvelope, XdrCodec,
};
use wallet::types::{FilterWith, TransactionFilterParam};

use crate::{oracle::*, Error};

fn is_vault(secret_key: &SecretKey, public_key: [u8; 32]) -> bool {
	return public_key == *secret_key.get_public().as_binary()
}

async fn listen_event<T>(
	parachain_rpc: &SpacewalkParachain,
) -> Result<Option<T>, ServiceError<Error>>
where
	T: StaticEvent + Debug,
{
	let (sender, mut receiver) = tokio::sync::mpsc::channel(10);

	parachain_rpc
		.on_event::<T, _, _, _>(
			|event| async {
				tracing::info!("Received Event: {:?}", event);

				if let Err(e) = sender.send(event).await {
					tracing::error!("failed to send issue id: {:?}", e);
					return
				}
			},
			|error| tracing::error!("Error: {:?}", error),
		)
		.await?;

	Ok(receiver.recv().await)
}

/// Listens for RequestIssueEvent directed at the vault.
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `vault_secret_key` - The secret key of this vault
/// * `issues` - a map to save all the new issue requests
pub async fn listen_for_issue_requests(
	parachain_rpc: SpacewalkParachain,
	vault_secret_key: SecretKey,
	issues: Arc<RwLock<IssueRequestsMap>>,
) -> Result<(), ServiceError<Error>> {
	match listen_event::<RequestIssueEvent>(&parachain_rpc).await? {
		Some(event) => {
			let event: RequestIssueEvent = event;
			if is_vault(&vault_secret_key, event.vault_stellar_public_key.clone()) {
				// let's get the IssueRequest
				let issue_request = parachain_rpc.get_issue_request(event.issue_id).await?;
				issues.write().await.insert(event.issue_id, issue_request);
			}
		},
		None => {
			tracing::trace!("Receiver didn't receive any Issue Request.");
		},
	}

	Ok(())
}

pub async fn listen_for_issue_cancels(
	parachain_rpc: SpacewalkParachain,
	issues: Arc<RwLock<IssueRequestsMap>>,
) -> Result<(), ServiceError<Error>> {
	match listen_event::<CancelIssueEvent>(&parachain_rpc).await? {
		Some(event) => {
			issues.write().await.remove(&event.issue_id);
		},
		None => {
			tracing::trace!("Receiver didn't receive any cancel issue event.");
		},
	}
	Ok(())
}

pub async fn listen_for_executed_issues(
	parachain_rpc: SpacewalkParachain,
	issues: Arc<RwLock<IssueRequestsMap>>,
) -> Result<(), ServiceError<Error>> {
	match listen_event::<ExecuteIssueEvent>(&parachain_rpc).await? {
		Some(event) => {
			issues.write().await.remove(&event.issue_id);
		},
		None => {
			tracing::trace!("Receiver didn't receive any execute issue event.");
		},
	}

	Ok(())
}

fn get_relevant_envelope(base64_xdr: &[u8]) -> Option<Transaction> {
	let xdr = base64::decode(base64_xdr).unwrap();

	match TransactionEnvelope::from_xdr(xdr) {
		Ok(res) => match res {
			TransactionEnvelope::EnvelopeTypeTx(env) => Some(env.tx),
			_ => None,
		},
		Err(e) => {
			tracing::error!("could not derive a TransactionEnvelope: {:?}", e);
			None
		},
	}
}

fn get_issue_id_from_tx(base64_xdr: &str) -> Option<IssueId> {
	let tx = get_relevant_envelope(base64_xdr.as_bytes())?;

	match tx.memo {
		Memo::MemoHash(issue_id) => Some(H256::from(issue_id)),
		_ => None,
	}
}

pub async fn process_issues_with_proofs(
	parachain_rpc: SpacewalkParachain,
	proof_ops: Arc<RwLock<dyn ProofExt>>,
	slot_tx_env_map: Arc<RwLock<HashMap<u32, String>>>,
	issues: Arc<RwLock<IssueRequestsMap>>,
) -> Result<(), ServiceError<Error>> {
	loop {
		let ops_clone = proof_ops.read().await;
		match ops_clone.get_pending_proofs().await {
			Ok(proofs) => {
				tokio::spawn(execute_issue(
					parachain_rpc.clone(),
					slot_tx_env_map.clone(),
					issues.clone(),
					proofs,
				));
			},
			Err(e) => {
				tracing::warn!("no proofs found just yet: {:?}", e);
			},
		}
	}
}

/// executes issue requests
///
/// # Arguments
///
/// * `parachain_rpc` - the parachain RPC handle
/// * `proofs` - a list of proofs to execute
pub async fn execute_issue(
	parachain_rpc: SpacewalkParachain,
	slot_tx_env_map: Arc<RwLock<HashMap<u32, String>>>,
	issues: Arc<RwLock<IssueRequestsMap>>,
	proofs: Vec<Proof>,
) -> Result<(), ServiceError<Error>> {
	let slot_tx_map = slot_tx_env_map.read().await;
	for proof in proofs {
		let (envelopes, tx_set) = proof.encode();

		let u32_slot = match u32::try_from(proof.slot()) {
			Ok(x) => x,
			Err(e) => {
				tracing::warn!("conversion problem: {:?}", e);
				continue
			},
		};

		let tx_env = match slot_tx_map.get(&u32_slot) {
			None => continue,
			Some(tx_env) => tx_env,
		};

		let issue_id = match get_issue_id_from_tx(tx_env) {
			Some(issue_id) if issues.read().await.contains_key(&issue_id) => issue_id,
			_ => continue,
		};

		match parachain_rpc
			.execute_issue(issue_id, tx_env.as_bytes(), envelopes.as_bytes(), tx_set.as_bytes())
			.await
		{
			Ok(_) => {},
			Err(err) if err.is_issue_completed() => {
				tracing::info!("Issue #{} has been completed", issue_id);
			},
			Err(err) => return Err(err.into()),
		}
	}

	Ok(())
}

#[derive(Clone)]
pub struct IssueFilter {
	vault_address: String,
}

impl IssueFilter {
	pub fn new(vault_public_key: &PublicKey) -> Result<Self, Error> {
		let encoding = vault_public_key.to_encoding();
		let x = std::str::from_utf8(&encoding)?;
		Ok(IssueFilter { vault_address: format!("{}", x) })
	}
}

fn check_asset(issue_asset: CurrencyId, tx_asset: Asset) -> bool {
	match primitives::AssetConversion::lookup(issue_asset) {
		Ok(issue_asset) => issue_asset != tx_asset,
		Err(e) => {
			tracing::warn!("Cannot convert to asset type: {:?}", e);
			false
		},
	}
}

fn is_tx_relevant(
	tx_xdr: &[u8],
	vault_address: &str,
	issue_asset: CurrencyId,
	issue_amount: u128,
) -> bool {
	// get envelope
	if let Some(transaction) = get_relevant_envelope(tx_xdr) {
		let payment_ops_to_vault_address: Vec<&PaymentOp> = transaction
			.operations
			.get_vec()
			.into_iter()
			.filter_map(|op| match &op.body {
				stellar_relay_lib::sdk::types::OperationBody::Payment(p) => {
					let d = p.destination.clone();
					if let Ok(d_address) = std::str::from_utf8(d.to_encoding().as_slice()) {
						if vault_address == d_address {
							return Some(p)
						}
					}
					None
				},
				_ => None,
			})
			.collect();

		if payment_ops_to_vault_address.len() > 0 {
			for payment_op in payment_ops_to_vault_address {
				// todo: check also the amount?
				let tx_amount = payment_op.amount;

				// check asset
				if check_asset(issue_asset, payment_op.asset.clone()) {
					return true
				}
			}
		}
	}

	// The transaction is not relevant to use since it doesn't
	// include a payment to our vault address
	false
}

impl FilterWith<TransactionFilterParam<IssueRequestsMap>> for IssueFilter {
	fn is_relevant(&self, param: TransactionFilterParam<IssueRequestsMap>) -> bool {
		let tx = param.0;
		let issue_requests = param.1;

		let memo_type = match String::from_utf8(tx.memo_type.clone()) {
			Ok(memo) => memo,
			Err(e) => {
				tracing::warn!("Failed to retrieve memo type: {:?}", e);
				return false
			},
		};

		// we only care about hash memo types.
		if memo_type != "hash" {
			return false
		}

		// get the issue_id from the memo field of tx.
		let issue_id = match &tx.memo {
			None => return false,
			Some(memo) => {
				// First decode the hex encoded memo to a vector of 32 bytes
				let decoded_memo = hex::decode(memo.clone());
				if decoded_memo.is_err() {
					return false
				}
				H256::from_slice(decoded_memo.unwrap().as_slice())
			},
		};

		// check if the issue_id is in the list.
		match issue_requests.get(&issue_id) {
			None => false,
			Some(request) => {
				// check if the transaction matches with the issue request
				is_tx_relevant(&tx.envelope_xdr, &self.vault_address, request.asset, request.amount)
			},
		}
	}
}
