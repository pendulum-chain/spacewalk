use std::sync::Arc;

use std::time::Duration;
use tokio::time::sleep;
use primitives::stellar::{Memo, Transaction, TransactionEnvelope, XdrCodec};
use primitives::TransactionEnvelopeExt;
use crate::{StellarWallet, TransactionResponse};
use crate::error::{CacheError, CacheErrorKind, Error};
use crate::error::Error::{DecodeError, ResubmissionError};

#[cfg(test)]
use mocktopus::macros::mockable;

#[cfg_attr(test, mockable)]
impl StellarWallet {

    /// Submits transactions found in the wallet's cache to Stellar.
    pub async fn resubmit_transactions_from_cache(&self) {
        let _ = self.transaction_submission_lock.lock().await;

        // Collect all envelopes from cache. Log those with errors.
        let envelopes =  match self.get_tx_envelopes_from_cache() {
            Ok((envs, errors)) => {
                tracing::warn!("resubmit_transactions_from_cache(): errors from cache: {errors:?}");
                println!("resubmit_transactions_from_cache(): errors from cache: {errors:?}");
                envs
            }
            Err(errors) => {
                tracing::warn!("resubmit_transactions_from_cache(): errors from cache: {errors:?}");
                println!("resubmit_transactions_from_cache(): errors from cache: {errors:?}");
                return
            }
        };

        let submit = |envelope:TransactionEnvelope| async {
            self.submit_transaction(envelope).await
        };

        // there's nothing to resubmit
        if envelopes.is_empty() { return }

        let mut error_collector = vec![];
        for envelope in envelopes {
            if let Err(e) = submit(envelope).await {
                println!("encountered error: {e:?}");
                error_collector.push(e);
            }
        }

        // handle errors found after submission
        if !error_collector.is_empty() {
            self.handle_errors(error_collector).await;
        }
    }

    /// Handle all errors
    async fn handle_errors(&self, errors:Vec<Error>) {
        let me = Arc::new(self.clone());

        for e in errors.into_iter() {
            let mut error = e;
            let x = error.to_string();
            let me_clone = Arc::clone(&me);
            tokio::spawn(async move {
                loop {
                    println!("handle_errors(): handle error: {error:?}");
                    match me_clone.handle_error(error).await {
                        Ok(res) => {
                            println!("handle_errors(): handled: {x:?} ");
                            return;
                        },
                        Err(e) => error = e
                    }
                    sleep(Duration::from_secs(1800)).await;
                }
            });
        }
    }

    /// Determines whether an error is up for resubmission or not:
    /// `tx_bad_seq` or `SequenceNumberAlreadyUsed` can be resubmitted by updating the sequence number
    /// `tx_internal_error` should be resubmitted again
    /// other errors must be logged and removed from cache.
    async fn handle_error(&self, error: Error) -> Result<(), Error> {
        match &error {
            Error::HorizonSubmissionError { reason, envelope_xdr, .. } => {
                match &reason[..] {
                    "tx_bad_seq" =>
                        return self.handle_tx_bad_seq_error_with_xdr(envelope_xdr).await
                            .map(|_| ()),
                    "tx_internal_error" =>
                        return self.handle_tx_internal_error(envelope_xdr).await
                            .map(|_| ()),
                    _ => {
                        if let Ok(env) = decode_to_envelope(envelope_xdr) {
                            if let Some(sequence) = env.sequence_number() {
                                if let Err(e) = self.remove_tx_envelope_from_cache(sequence) {
                                    tracing::warn!("handle_error():: failed to remove transaction with sequence {sequence}: {e:?}");
                                }
                            }
                        };
                        tracing::error!(
							"handle_error(): Unrecoverable HorizonSubmissionError: {error:?}"
						);
                    },
                }
            }
            Error::CacheError(CacheError {
                                  kind: CacheErrorKind::SequenceNumberAlreadyUsed,
                                  envelope,
                                  ..
                              }) =>
                if let Some(transaction_envelope) = envelope {
                    return self.handle_tx_bad_seq_error_with_envelope(transaction_envelope.clone()).await
                        .map(|_| ());
                } else {
                    tracing::warn!(
						"handle_error(): SequenceNumberAlreadyUsed error but no envelope"
					);
                    println!(
						"handle_error(): SequenceNumberAlreadyUsed error but no envelope"
					);
                },
            _ => {
                tracing::warn!("handle_error(): Unrecoverable error in Stellar wallet: {error:?}");
            },
        }

        Ok(())
    }

    async fn handle_tx_internal_error(&self, envelope_xdr_as_str_opt:&Option<String>) -> Result<TransactionResponse,Error> {
       let mut envelope =  decode_to_envelope(envelope_xdr_as_str_opt)?;
        self.sign_envelope(&mut envelope)?;

        self.submit_transaction(envelope).await
    }
}

// handle tx_bad_seq
#[cfg_attr(test, mockable)]
impl StellarWallet {
    async fn handle_tx_bad_seq_error_with_xdr(&self, envelope_xdr_as_str_opt:&Option<String>) -> Result<TransactionResponse,Error> {
        let Some( envelope_xdr) = envelope_xdr_as_str_opt else {
            tracing::warn!("handle_tx_bad_seq_error_with_xdr(): tx_bad_seq error but no envelope_xdr");

            println!("handle_tx_bad_seq_error_with_xdr(): tx_bad_seq error but no envelope_xdr");
            return Err(ResubmissionError(
                "tx_bad_seq error but no envelope_xdr".to_string(),
            ))
        };

        let tx_envelope =
            TransactionEnvelope::from_base64_xdr(envelope_xdr)
                .map_err(|_| DecodeError)?;

        tracing::info!(
			"handle_tx_bad_seq_error_with_xdr(): tx_bad_seq error. Resubmitting {envelope_xdr}");

        println!(
			"handle_tx_bad_seq_error_with_xdr(): tx_bad_seq error. Resubmitting {envelope_xdr}");
        self.handle_tx_bad_seq_error_with_envelope(tx_envelope).await
    }

    async fn handle_tx_bad_seq_error_with_envelope(
        &self,
        tx_envelope: TransactionEnvelope,
    ) -> Result<TransactionResponse, Error> {
        let tx = tx_envelope.get_transaction().ok_or(DecodeError)?;
        println!("handle_tx_bad_seq_error_with_envelope(): time to process transaction: {}",tx.seq_num);

        // Check if we already submitted this transaction
        if !self.is_transaction_already_submitted(&tx).await {
            return self.bump_sequence_number_and_submit(tx).await
        } else {
            tracing::error!("handle_tx_bad_seq_error_with_envelope(): Similar transaction already submitted. Skipping {:?}", tx);
            println!("handle_tx_bad_seq_error_with_envelope(): Similar transaction already submitted. Skipping {:?}", tx);
            Err(ResubmissionError("Transaction already submitted".to_string()))
        }
    }

    async fn bump_sequence_number_and_submit(
        &self,
        tx: Transaction,
    ) -> Result<TransactionResponse, Error> {
        let sequence_number = self.get_sequence().await?;

        println!("Old sequence number: {}", tx.seq_num);
        let mut updated_tx = tx.clone();
        updated_tx.seq_num = sequence_number + 1 ;

        println!("New sequence number: {}", updated_tx.seq_num);

        let envelope = self.sign_and_create_envelope(updated_tx)?;

        self.submit_transaction(envelope).await
    }

    /// This function iterates over all transactions of an account to see if a similar transaction
    /// i.e. a transaction containing the same memo was already submitted previously.
    /// TODO: This operation is very costly and we should try to optimize it in the future.
    async fn is_transaction_already_submitted(&self, tx: &Transaction) -> bool {

        println!("is_transaction_already_submitted(): start finding...");

        let mut remaining_page = 10;
        let own_public_key = self.public_key();

        while let Ok(transaction) = self.get_all_transactions_iter().await {

            println!("is_transaction_already_submitted(): analyze:");

            if remaining_page == 0 {
                break
            }

            for response in transaction.records {
                // Make sure that we are the sender and not the receiver because otherwise an
                // attacker could send a transaction to us with the target memo and we'd wrongly
                // assume that we already submitted this transaction.
                let Ok(source_account) = response.source_account() else {
                    continue
                };
                if !source_account.eq(&own_public_key) {
                    continue
                }

                // Check that the transaction contains the memo that we want to send.
                let Some(response_memo) = response.memo_text() else { continue };
                let Memo::MemoText(tx_memo) = &tx.memo else { continue };

                if are_memos_eq(response_memo, tx_memo.get_vec()) {
                    println!("is_transaction_already_submitted(): found a match!");
                    return true
                }
            }
            remaining_page -= 1;
        }

        println!("is_transaction_already_submitted(): found NO match!");
        // We did not find a transaction that matched our criteria
        false
    }
}

#[cfg_attr(test, mockable)]
fn are_memos_eq(memo1: &Vec<u8>, memo2: &Vec<u8>) -> bool {
    memo1 == memo2
}

fn decode_to_envelope(envelope_xdr_as_str_opt:&Option<String>) -> Result<TransactionEnvelope, Error> {
    let Some( envelope_xdr) = envelope_xdr_as_str_opt else {
        tracing::warn!("handle_error(): tx_bad_seq error but no envelope_xdr");
        return Err(ResubmissionError(
            "tx_bad_seq error but no envelope_xdr".to_string(),
        ))
    };

    TransactionEnvelope::from_base64_xdr(envelope_xdr).map_err(|_| DecodeError)
}

#[cfg(test)]
mod test {
    use mocktopus::mocking::{Mockable, MockResult};
    use primitives::stellar::{Asset as StellarAsset, TransactionEnvelope, XdrCodec};
    use primitives::TransactionEnvelopeExt;
    use crate::mock::*;
    use crate::StellarWallet;
    use serial_test::serial;
    #[tokio::test]
    #[serial]
    async fn resubmit_transactions_works() {
        let wallet = wallet_with_storage("resources/resubmit_transactions_works")
            .expect("should return an arc rwlock wallet")
            .clone();
        let wallet = wallet.write().await;

        // let's send a successful transaction first

        let asset = StellarAsset::native();
        let amount = 1001;

        let seq_number = wallet.get_sequence().await.expect("should return a sequence");

        // creating a `tx_bad_seq` envelope.
        let bad_request_id: [u8;32] = rand::random();
        let bad_envelope = wallet
            .create_payment_envelope(
                default_destination(),
                asset.clone(),
                amount,
                bad_request_id,
                DEFAULT_STROOP_FEE_PER_OPERATION,
                seq_number+5,
            )
            .expect("should return an envelope");

        // let's save this in storage
        let _ = wallet.save_tx_envelope_to_cache(bad_envelope.clone()).expect("should save.");

        // create a successful transaction
        let good_request_id: [u8;32] = rand::random();
        let good_envelope = wallet
            .create_payment_envelope(
                default_destination(),
                asset,
                amount,
                good_request_id,
                DEFAULT_STROOP_FEE_PER_OPERATION,
                seq_number + 1,
            )
            .expect("should return an envelope");

        StellarWallet::is_transaction_already_submitted
            .mock_safe(move |_, _| MockResult::Return(Box::pin(async move { false })));

        // let's save this in storage
        let _ = wallet.save_tx_envelope_to_cache(good_envelope.clone()).expect("should save");

        // let's resubmit these 2 transactions
        wallet.resubmit_transactions_from_cache().await;

        let actual_sequence = wallet.get_sequence().await
            .expect("should return a sequence number");


        // 1 should pass, and 1 should fail.
        assert_eq!(actual_sequence, seq_number+1);

        wallet.remove_cache_dir();
    }


    #[tokio::test]
    async fn handle_error_for_tx_bad_seq() {
        let wallet = wallet_with_storage("resources/handle_error_works")
            .expect("should return an arc rwlock wallet")
            .clone();
        let mut wallet = wallet.write().await;

        // let's send a successful transaction first
        let asset = StellarAsset::native();
        let amount = 1001;
        let request_id = [0u8; 32];

        let response = wallet
            .send_payment_to_address(
                default_destination(),
                asset.clone(),
                amount,
                request_id,
                DEFAULT_STROOP_FEE_PER_OPERATION,
                false,
            )
            .await
            .expect("should be ok");

        // get the sequence number of the previous one.
        let env =
            TransactionEnvelope::from_base64_xdr(response.envelope_xdr).expect("should convert ok");
        let seq_number = env.sequence_number().expect("should return sequence number");

        // creating a `tx_bad_seq` envelope.
        let bad_request_id: [u8; 32] = rand::random();
        let bad_envelope = wallet
            .create_payment_envelope(
                default_destination(),
                asset.clone(),
                amount,
                bad_request_id,
                DEFAULT_STROOP_FEE_PER_OPERATION,
                seq_number,
            )
            .expect("should return an envelope");

        let Err(error) = wallet.submit_transaction(bad_envelope).await else {
            println!("oh my, it passed");
            panic!("failed!!");

        };

        println!("The wallet error: {error:?}");

        // Set this to false so that the wallet will try to resubmit the transaction.
        StellarWallet::is_transaction_already_submitted
            .mock_safe(move |_, _| MockResult::Return(Box::pin(async move { false })));

        let result = wallet.handle_error(error).await;

        // We assume that the transaction was re-created and submitted successfully.
        println!("AND THE RESULT IS: {result:?}");
        assert!(result.is_ok());


        // let response = result.unwrap();
        // assert_eq!(response.successful, true);
        //
        // // Try to handle the same error but this time the transaction was already submitted.
        // let submission_error = Error::HorizonSubmissionError {
        //     title: "title".to_string(),
        //     status: 400,
        //     reason: "tx_bad_seq".to_string(),
        //     envelope_xdr: envelope_xdr,
        // };
        //
        // StellarWallet::is_transaction_already_submitted
        //     .mock_safe(move |_, _| MockResult::Return(Box::pin(async move { true })));
        //
        // let result = wallet.handle_error(submission_error).await;
        // assert!(result.is_err());

        wallet.remove_cache_dir();
    }
}