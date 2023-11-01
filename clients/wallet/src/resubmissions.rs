use std::sync::Arc;

use crate::{
	error::{
		CacheError, CacheErrorKind, Error,
		Error::{DecodeError, ResubmissionError},
	},
	StellarWallet,
};
use primitives::{
	stellar::{Memo, Transaction, TransactionEnvelope, XdrCodec},
	TransactionEnvelopeExt,
};
use std::time::Duration;
use tokio::time::sleep;

#[cfg(test)]
use mocktopus::macros::mockable;
use tokio::sync::RwLock;

#[cfg_attr(test, mockable)]
impl StellarWallet {
	pub async fn resubmit_transactions_from_cache(
		&self,
	) -> Arc<RwLock<Vec<tokio::sync::oneshot::Receiver<bool>>>> {
		// a collection of all tasks currently running
		let running_tasks = Arc::new(RwLock::new(vec![]));

		// perform the resubmission
		self._resubmit_transactions_from_cache(running_tasks.clone()).await;

		// // clone the task, to share this in another thread
		// let running_tasks_clone = running_tasks.clone();
		//
		// // clone self, to be able to use this in another thread
		// let me = Arc::new(self.clone());
		// // spawn a thread to resubmit envelopes from cache
		// tokio::spawn(async move {
		//     let me_clone = Arc::clone(&me);
		//     loop {
		//         println!("time for spawning");
		//         pause_process_in_secs(300).await;
		//
		//         println!("let's gooo");
		//         me_clone._resubmit_transactions_from_cache(running_tasks_clone.clone()).await;
		//     }
		// });

		// return the list of tasks, to monitor the resubmission
		running_tasks
	}

	/// Submits transactions found in the wallet's cache to Stellar.
	async fn _resubmit_transactions_from_cache(
		&self,
		running_tasks: Arc<RwLock<Vec<tokio::sync::oneshot::Receiver<bool>>>>,
	) {
		let _ = self.transaction_submission_lock.lock().await;

		// Collect all envelopes from cache. Log those with errors.
		let envelopes = match self.get_tx_envelopes_from_cache() {
			Ok((envs, errors)) => {
				tracing::warn!(
					"_resubmit_transactions_from_cache(): errors from cache: {errors:?}"
				);
				envs
			},
			Err(errors) => {
				tracing::warn!(
					"_resubmit_transactions_from_cache(): errors from cache: {errors:?}"
				);
				return
			},
		};

		// to prevent `error[E0434]: can't capture dynamic environment in a fn item`,
		// use a closure instead
		let submit =
			|envelope: TransactionEnvelope| async { self.submit_transaction(envelope).await };

		// there's nothing to resubmit
		if envelopes.is_empty() {
			return
		}

		let mut error_collector = vec![];
		for envelope in envelopes {
			if let Err(e) = submit(envelope).await {
				tracing::debug!("_resubmit_transactions_from_cache(): encountered error: {e:?}");
				error_collector.push(e);
			}
		}

		// clone self, to use in another thread
		let me = Arc::new(self.clone());

		tokio::spawn(async move {
			// handle errors found after submission
			if !error_collector.is_empty() {
				me.handle_errors(error_collector, running_tasks).await;
			}
		});
	}

	/// Handle all errors
	async fn handle_errors(
		&self,
		mut errors: Vec<Error>,
		running_tasks: Arc<RwLock<Vec<tokio::sync::oneshot::Receiver<bool>>>>,
	) {
		while let Some(error) = errors.pop() {
			// pause process for 20 minutes
			#[cfg(not(test))]
			pause_process_in_secs(1200).await;

			// for testig purpose, set to 5 seconds
			#[cfg(test)]
			pause_process_in_secs(5).await;

			// checks whether the resubmission is done, regardless if it was successful or not
			let (sender, receiver) = tokio::sync::oneshot::channel();
			// save the receivers, to determine the finished tasks.
			{
				let mut writer = running_tasks.write().await;
				writer.push(receiver);
			}

			// handle the error
			match self.handle_error(error).await {
				// Successful or not, this error is considered "handled".
				Ok(_) =>
					if let Err(_) = sender.send(true) {
						println!("failed to send true message");
					},
				// a new kind of error occurred. Process it on the next loop.
				Err(e) => {
					println!("handle_errors(): error happened again: {e:?}");
					errors.push(e);
				},
			}
		}
	}

	/// Determines whether an error is up for resubmission or not:
	/// `tx_bad_seq` or `SequenceNumberAlreadyUsed` can be resubmitted by updating the sequence
	/// number `tx_internal_error` should be resubmitted again
	/// other errors must be logged and removed from cache.
	async fn handle_error(&self, error: Error) -> Result<(), Error> {
		match &error {
			Error::HorizonSubmissionError { reason, envelope_xdr, .. } => match &reason[..] {
				"tx_bad_seq" => return self.handle_tx_bad_seq_error_with_xdr(envelope_xdr).await,
				"tx_internal_error" => return self.handle_tx_internal_error(envelope_xdr).await,
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
			},
			Error::CacheError(CacheError {
				kind: CacheErrorKind::SequenceNumberAlreadyUsed,
				envelope,
				..
			}) => {
				if let Some(transaction_envelope) = envelope {
					return self
						.handle_tx_bad_seq_error_with_envelope(transaction_envelope.clone())
						.await
				}

				tracing::warn!("handle_error(): SequenceNumberAlreadyUsed error but no envelope");
			},
			_ => tracing::warn!("handle_error(): Unrecoverable error in Stellar wallet: {error:?}"),
		}

		Ok(())
	}

	async fn handle_tx_internal_error(
		&self,
		envelope_xdr_as_str_opt: &Option<String>,
	) -> Result<(), Error> {
		let mut envelope = decode_to_envelope(envelope_xdr_as_str_opt)?;
		self.sign_envelope(&mut envelope)?;

		self.submit_transaction(envelope).await.map(|resp| {
			tracing::info!("handle_tx_internal_error(): response: {resp:?}");
			()
		})
	}
}

// handle tx_bad_seq
#[cfg_attr(test, mockable)]
impl StellarWallet {
	async fn handle_tx_bad_seq_error_with_xdr(
		&self,
		envelope_xdr_as_str_opt: &Option<String>,
	) -> Result<(), Error> {
		let tx_envelope = decode_to_envelope(envelope_xdr_as_str_opt)?;
		self.handle_tx_bad_seq_error_with_envelope(tx_envelope).await
	}

	async fn handle_tx_bad_seq_error_with_envelope(
		&self,
		tx_envelope: TransactionEnvelope,
	) -> Result<(), Error> {
		let tx = tx_envelope.get_transaction().ok_or(DecodeError)?;
		println!(
			"handle_tx_bad_seq_error_with_envelope(): time to process transaction: {}",
			tx.seq_num
		);

		// Check if we already submitted this transaction
		if !self.is_transaction_already_submitted(&tx).await {
			println!(
				"handle_tx_bad_seq_error_with_envelope(): tx did not exist before: {}",
				tx.seq_num
			);
			return self.bump_sequence_number_and_submit(tx).await.map(|resp| {
				tracing::info!("handle_tx_bad_seq_error_with_envelope(): response: {resp:?}");
				()
			})
		}

		tracing::error!("handle_tx_bad_seq_error_with_envelope(): Similar transaction already submitted. Skipping {:?}", tx);
		println!("handle_tx_bad_seq_error_with_envelope(): Similar transaction already submitted. Skipping {:?}", tx);

		// remove from cache
		if let Err(e) = self.remove_tx_envelope_from_cache(tx.seq_num) {
			tracing::warn!("handle_tx_bad_seq_error_with_envelope(): failed to remove envelope with sequence {} in cache: {e:?}", tx.seq_num);
		}

		Err(ResubmissionError("Transaction already submitted".to_string()))
	}

	async fn bump_sequence_number_and_submit(&self, tx: Transaction) -> Result<(), Error> {
		let sequence_number = self.get_sequence().await?;

		println!("Old sequence number: {}", tx.seq_num);
		let mut updated_tx = tx.clone();
		updated_tx.seq_num = sequence_number + 1;

		println!("New sequence number: {}", updated_tx.seq_num);

		let envelope = self.sign_and_create_envelope(updated_tx)?;
		self.submit_transaction(envelope).await.map(|resp| {
			tracing::info!("bump_sequence_number_and_submit(): response: {resp:?}");
			()
		})
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

#[cfg_attr(test, mockable)]
async fn pause_process_in_secs(in_secs: u64) {
	sleep(Duration::from_secs(in_secs)).await;
}

fn decode_to_envelope(
	envelope_xdr_as_str_opt: &Option<String>,
) -> Result<TransactionEnvelope, Error> {
	let Some( envelope_xdr) = envelope_xdr_as_str_opt else {
        tracing::warn!("handle_error(): no envelope_xdr found");
        return Err(ResubmissionError("no envelope_xdr".to_string()))
    };

	TransactionEnvelope::from_base64_xdr(envelope_xdr).map_err(|_| DecodeError)
}

#[cfg(test)]
mod test {
	use crate::{mock::*, resubmissions::pause_process_in_secs, StellarWallet};
	use mocktopus::mocking::{MockResult, Mockable};
	use primitives::{
		stellar::{Asset as StellarAsset, TransactionEnvelope, XdrCodec},
		TransactionEnvelopeExt,
	};
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
		let bad_request_id: [u8; 32] = rand::random();
		let bad_envelope = wallet
			.create_payment_envelope(
				default_destination(),
				asset.clone(),
				amount,
				bad_request_id,
				DEFAULT_STROOP_FEE_PER_OPERATION,
				seq_number + 5,
			)
			.expect("should return an envelope");

		// let's save this in storage
		let _ = wallet.save_tx_envelope_to_cache(bad_envelope.clone()).expect("should save.");

		// create a successful transaction
		let good_request_id: [u8; 32] = rand::random();
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
		let x = wallet.resubmit_transactions_from_cache().await;

		loop {
			println!("start the loop");
			pause_process_in_secs(30).await;
			let mut res = x.write().await;

			if res.is_empty() {
				break
			}

			let mut receiver = res.pop().expect("should return something");

			if let Err(_) = receiver.try_recv() {
				res.push(receiver);
			}
		}

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
