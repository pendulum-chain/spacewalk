#![allow(dead_code)]

mod actor;
mod collector;
mod constants;
mod errors;
mod handler;
mod storage;
mod types;

use std::{collections::HashMap, time::Duration};

use sp_keyring::AccountKeyring;
use stellar_relay::sdk::{
    compound_types::UnlimitedVarArray,
    network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
    types::{PaymentOp, ScpEnvelope, StellarMessage, TransactionSet},
    Asset, Memo, SecretKey, Transaction, TransactionEnvelope,
};

use crate::stellar_oracle::actor::{Actor, ActorHandler};
use collector::*;
use constants::*;
use errors::Error;
use handler::*;
use stellar_relay::{connect, node::NodeInfo, ConnConfig, StellarNodeMessage, UserControls};
use storage::{handler::*, traits::*};
use types::*;

pub trait FilterWith {
    fn is_tx_relevant(&self, tx: &Transaction) -> bool;
}

#[derive(Copy, Clone)]
pub struct FilterTx;

impl FilterWith for FilterTx {
    fn is_tx_relevant(&self, transaction: &Transaction) -> bool {
        let payment_ops_to_vault_address: Vec<&PaymentOp> = transaction
            .operations
            .get_vec()
            .into_iter()
            .filter_map(|op| match &op.body {
                stellar_relay::sdk::types::OperationBody::Payment(p) => {
                    let d = p.destination.clone();
                    if VAULT_ADDRESSES_FILTER.contains(&std::str::from_utf8(d.to_encoding().as_slice()).unwrap()) {
                        Some(p)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        if payment_ops_to_vault_address.len() == 0 {
            // The transaction is not relevant to use since it doesn't
            // include a payment to our vault address
            return false;
        } else {
            tracing::info!("Transaction to our vault address received.");
            let source = transaction.source_account.clone();
            for payment_op in payment_ops_to_vault_address {
                let destination = payment_op.destination.clone();
                let amount = payment_op.amount;
                let asset = payment_op.asset.clone();
                tracing::info!("Deposit amount {:?} stroops", amount);
                // print_asset(asset);
                tracing::info!(
                    "From {:#?}",
                    std::str::from_utf8(source.to_encoding().as_slice()).unwrap()
                );
                tracing::info!(
                    "To {:?}",
                    std::str::from_utf8(destination.to_encoding().as_slice()).unwrap()
                );
            }
            return true;
        }
    }
}

fn print_asset(asset: Asset) {
    match asset {
        Asset::AssetTypeNative => tracing::info!("XLM"),
        Asset::AssetTypeCreditAlphanum4(value) => {
            tracing::info!("{:?}", std::str::from_utf8(value.asset_code.as_slice()).unwrap());
            tracing::info!(
                "{:?}",
                std::str::from_utf8(value.issuer.to_encoding().as_slice()).unwrap()
            );
        }
        Asset::AssetTypeCreditAlphanum12(value) => {
            tracing::info!("{:?}", std::str::from_utf8(value.asset_code.as_slice()).unwrap());
            tracing::info!(
                "{:?}",
                std::str::from_utf8(value.issuer.to_encoding().as_slice()).unwrap()
            );
        }
        Asset::Default(code) => tracing::info!("Asset type {:?}", code),
    }
}

/// Returns an ActorHandler that will allow us to get the map.
/// ```
/// loop {
/// 		tokio::time::sleep(Duration::from_secs(8)).await;
///
/// 		let (sender,receiver) = tokio::sync::oneshot::channel();
/// 		handler.get_map(sender).await?;
///
/// 		if let Ok(res) = receiver.await {
/// 			log::info!("the map {:?}", res.envelopes_map().len());
/// 		}
/// 	}
/// ```
pub async fn handler(node_info: NodeInfo, connection_cfg: ConnConfig) -> Result<ActorHandler, Error> {
    prepare_directories()?;

    let mut collector = ScpMessageCollector::new(connection_cfg.is_public_network);
    let mut user: UserControls = connect(node_info, connection_cfg).await?;

    Ok(ActorHandler::new(user, collector))
}

#[cfg(test)]
mod test {
    use std::env;

    use stellar_relay::sdk::types::ScpStatementPledges;

    use stellar_relay::helper::compute_non_generic_tx_set_content_hash;

    use super::*;

    #[test]
    fn find_file_successful() {
        let slot = 42867089;

        let file_name = EnvelopesFileHandler::find_file_by_slot(slot).expect("should return a file");
        assert_eq!(&file_name, "42867088_42867102");

        let file_name = TxSetsFileHandler::find_file_by_slot(slot).expect("should return a file");
        assert_eq!(&file_name, "42867088_42867102");

        let file_name = TxHashesFileHandler::find_file_by_slot(slot).expect("should return a file");
        assert_eq!(&file_name, "42867088_42867102");

        let slot = 42867150;

        let file_name = EnvelopesFileHandler::find_file_by_slot(slot).expect("should return a file");
        assert_eq!(&file_name, "42867148_42867162");

        let file_name = TxSetsFileHandler::find_file_by_slot(slot).expect("should return a file");
        assert_eq!(&file_name, "42867135_42867150");

        let file_name = TxHashesFileHandler::find_file_by_slot(slot).expect("should return a file");
        assert_eq!(&file_name, "42867135_42867150");

        let slot = 42990037;
        let file_name = EnvelopesFileHandler::find_file_by_slot(slot).expect("should return a file");
        assert_eq!(&file_name, "42990036_42990037");
    }

    #[test]
    fn read_file_successful() {
        let first_slot = 42867118;
        let last_slot = 42867132;

        let envelopes_map =
            EnvelopesFileHandler::read_file(&format!("{}_{}", first_slot, last_slot)).expect("should return a map");

        for (idx, slot) in envelopes_map.keys().enumerate() {
            let expected_slot_num = first_slot + u64::try_from(idx).expect("should return u64 data type");
            assert_eq!(slot, &expected_slot_num);
        }

        let scp_envelopes = envelopes_map.get(&last_slot).expect("should have scp envelopes");

        for x in scp_envelopes {
            assert_eq!(x.statement.slot_index, last_slot);
        }

        let filename = TxSetsFileHandler::find_file_by_slot(last_slot).expect("should return a filename");
        let txset_map = TxSetsFileHandler::read_file(&filename).expect("should return a txset map");

        let txset = txset_map.get(&last_slot).expect("should have a txset");
        let tx_set_hash = compute_non_generic_tx_set_content_hash(txset);

        let first = scp_envelopes.first().expect("should return an envelope");

        if let ScpStatementPledges::ScpStExternalize(stmt) = &first.statement.pledges {
            let expected_tx_set_hash = get_tx_set_hash(&stmt).expect("return a tx set hash");

            assert_eq!(tx_set_hash, expected_tx_set_hash);
        } else {
            assert!(false);
        }

        let txes = txset.txes.get_vec();
        let idx = txes.len() / 2;
        let tx = txes.get(idx).expect("should return a tx envelope.");

        let network = Network::new(b"Public Global Stellar Network ; September 2015");

        let hash = tx.get_hash(&network);
        let txhash_map = TxHashesFileHandler::read_file(&filename).expect("should return txhash map");
        let actual_slot = txhash_map.get(&hash).expect("should return a slot number");
        assert_eq!(actual_slot, &last_slot);
    }
}
