use stellar_relay::sdk::XdrCodec;

use super::*;

pub struct Proof {
    tx_env: TransactionEnvelope,
    envelopes: UnlimitedVarArray<ScpEnvelope>,
    tx_set: TransactionSet,
}

// Returns a bool indicating whether the transaction was successfully handled or not
pub async fn handle_tx(tx_env: TransactionEnvelope, collector: &ScpMessageCollector) -> Result<bool, Error> {
    if let Some((envelopes, txset)) = build_proof(&tx_env, collector) {
        if collector.is_public() {
            if envelopes.len() < 20 {
                tracing::info!(
                    "Not yet enough envelopes to build proof, current amount {:?}. Retrying in next loop...",
                    envelopes.len()
                );
                return Ok(false);
            }
        } else if envelopes.len() < 2 {
            tracing::info!(
                "Not yet enough envelopes to build proof, current amount {:?}. Retrying in next loop...",
                envelopes.len()
            );
            return Ok(false);
        }

        // tracing::info!("Sending proof for tx: {:?} with {:?} scp messages", tx_env.get_hash(collector.network()),
        // envelopes.len()); let (tx_env, envelopes, txset) = encode(tx_env, envelopes, txset);
        // let tx = spacewalk_chain::tx()
        //     .stellar_relay()
        //     .validate_stellar_transaction_ext(
        //         tx_env.as_bytes().to_vec(),
        //         envelopes.as_bytes().to_vec(),
        //         txset.as_bytes().to_vec(),
        //         collector.is_public(),
        //     );
        // let signer = PairSigner::new(AccountKeyring::Alice.pair());
        // let hash = api.tx().sign_and_submit_default(&tx, &signer).await?;
        // tracing::info!("Successfully submitted validate_stellar_transaction_ext() extrinsic: {:?}", hash);
    }

    Ok(true)
}

fn encode(
    tx_env: TransactionEnvelope,
    envelopes: UnlimitedVarArray<ScpEnvelope>,
    tx_set: TransactionSet,
) -> (String, String, String) {
    let tx_env_xdr = tx_env.to_xdr();
    let tx_env_encoded = base64::encode(tx_env_xdr);

    let envelopes_xdr = envelopes.to_xdr();
    let envelopes_encoded = base64::encode(envelopes_xdr);

    let tx_set_xdr = tx_set.to_xdr();
    let tx_set_encoded = base64::encode(tx_set_xdr);

    (tx_env_encoded, envelopes_encoded, tx_set_encoded)
}

fn build_proof(
    tx_env: &TransactionEnvelope,
    collector: &ScpMessageCollector,
) -> Option<(UnlimitedVarArray<ScpEnvelope>, TransactionSet)> {
    let tx_hash = tx_env.get_hash(collector.network());

    let tx_hash_map = collector.tx_hash_map();
    let slot = tx_hash_map.get(&tx_hash)?;

    let txset_map = collector.txset_map();

    let tx_set = txset_map.get(slot).cloned().or_else(|| {
        if let Ok(m) = TxSetsFileHandler::get_map_from_archives(*slot) {
            m.get(slot).cloned()
        } else {
            None
        }
    })?;

    let envelopes = collector.envelopes_map().get(slot).cloned().or_else(|| {
        if let Ok(envelopes_map) = EnvelopesFileHandler::get_map_from_archives(*slot) {
            envelopes_map.get(slot).cloned()
        } else {
            None
        }
    })?;
    let envelopes = UnlimitedVarArray::new(envelopes.clone()).unwrap_or(UnlimitedVarArray::new_empty());

    Some((envelopes, tx_set))
}
