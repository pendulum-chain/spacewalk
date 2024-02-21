use std::sync::Arc;

use crate::{
	error::{
		CacheError, CacheErrorKind, Error,
		Error::{DecodeError, ResubmissionError},
	},
	StellarWallet, TransactionResponse,
};
use primitives::{
	stellar::{Memo, Transaction, TransactionEnvelope, XdrCodec},
	TransactionEnvelopeExt,
};
use std::time::Duration;
use tokio::time::sleep;

use crate::horizon::{responses::TransactionsResponseIter, DEFAULT_PAGE_SIZE};
#[cfg(test)]
use mocktopus::macros::mockable;
use primitives::stellar::{types::SequenceNumber, PublicKey};
use reqwest::Client;

pub const RESUBMISSION_INTERVAL_IN_SECS: u64 = 1800;

#[cfg_attr(test, mockable)]
impl StellarWallet {
	pub async fn start_periodic_resubmission_of_transactions_from_cache(
		&self,
		interval_in_seconds: u64,
	) {
		// Perform the resubmission
		self._resubmit_transactions_from_cache().await;

		// The succeeding resubmission will be done on intervals
		// Clone self to use this in another thread
		let me = Arc::new(self.clone());
		// Spawn a thread to resubmit envelopes from cache
		tokio::spawn(async move {
			let me_clone = Arc::clone(&me);
			loop {
				pause_process_in_secs(interval_in_seconds).await;

				me_clone._resubmit_transactions_from_cache().await;
			}
		});
	}

	#[doc(hidden)]
	/// Submits transactions found in the wallet's cache to Stellar.
	async fn _resubmit_transactions_from_cache(&self) {
		let _ = self.transaction_submission_lock.lock().await;

		// Collect envelopes from cache
		let envelopes = match self.get_tx_envelopes_from_cache() {
			Ok((envs, errors)) => {
				//  Log those with errors.
				if !errors.is_empty() {
					tracing::warn!(
						"_resubmit_transactions_from_cache(): errors from cache: {errors:?}"
					);
				}
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
		tracing::info!(
			"_resubmit_transactions_from_cache(): resubmitting {:?} envelopes in cache...",
			envelopes.len()
		);

		let mut error_collector = vec![];
		// loop through the envelopes and resubmit each one
		for envelope in envelopes {
			if let Err(e) = submit(envelope.clone()).await {
				tracing::debug!("_resubmit_transactions_from_cache(): encountered error: {e:?}");
				// save the kind of error and the envelope that failed
				error_collector.push((e, envelope));
			}
		}

		// a few errors happened and must be handled.
		if !error_collector.is_empty() {
			// clone self, to use in another thread
			let me = Arc::new(self.clone());
			tokio::spawn(async move {
				me.handle_errors(error_collector).await;
			});
		}
	}

	#[doc(hidden)]
	/// Handle all errors
	///
	/// # Arguments
	/// * `errors` - a list of a tuple containing the error and its transaction envelope
	async fn handle_errors(
		&self,
		#[allow(unused_mut)] mut errors: Vec<(Error, TransactionEnvelope)>,
	) {
		while let Some((error, env)) = errors.pop() {
			// handle the error
			match self.handle_error(error).await {
				// a new kind of error occurred. Process it on the next loop.
				Err(e) => {
					tracing::error!("handle_errors(): new error occurred: {e:?}");

					// push the transaction that failed, and the corresponding error
					errors.push((e, env));
				},

				// Resubmission failed for this Transaction Envelope and it's a non-recoverable
				// error Remove from cache
				Ok(None) => self.remove_tx_envelope_from_cache(&env),

				// Resubmission was successful
				Ok(Some(resp)) =>
					tracing::debug!("handle_errors(): successfully processed envelope: {resp:?}"),
			}
		}
	}

	/// Returns:
	/// * `TransactionResponse` for successful resubmission;
	/// * None for errors that cannot be resubmitted;
	/// * An error that can potentially be resubmitted again
	///
	/// This function determines whether an error is up for resubmission or not:
	/// `tx_bad_seq` or `SequenceNumberAlreadyUsed` can be resubmitted by updating the sequence
	/// number `tx_internal_error` should be resubmitted again
	/// other errors must be logged and removed from cache.
	async fn handle_error(&self, error: Error) -> Result<Option<TransactionResponse>, Error> {
		match &error {
			Error::HorizonSubmissionError { reason, envelope_xdr, .. } => match &reason[..] {
				"tx_bad_seq" =>
					return self.handle_tx_bad_seq_error_with_xdr(envelope_xdr).await.map(Some),
				"tx_internal_error" =>
					return self.handle_tx_internal_error(envelope_xdr).await.map(Some),
				_ => {
					if let Ok(env) = decode_to_envelope(envelope_xdr) {
						self.remove_tx_envelope_from_cache(&env);
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
						.map(Some)
				}

				tracing::warn!("handle_error(): SequenceNumberAlreadyUsed error but no envelope");
			},
			_ => tracing::warn!("handle_error(): Unrecoverable error in Stellar wallet: {error:?}"),
		}

		// the error found is not recoverable, and cannot be resubmitted again.
		Ok(None)
	}

	// We encountered an unknown error and try submitting the transaction again as is
	async fn handle_tx_internal_error(
		&self,
		envelope_xdr_as_str_opt: &Option<String>,
	) -> Result<TransactionResponse, Error> {
		let mut envelope = decode_to_envelope(envelope_xdr_as_str_opt)?;
		self.sign_envelope(&mut envelope)?;

		self.submit_transaction(envelope).await
	}
}

fn is_source_account_match(public_key: &PublicKey, tx: &TransactionResponse) -> bool {
	match tx.source_account() {
		Err(_) => false,
		Ok(source_account) if !source_account.eq(&public_key) => false,
		_ => true,
	}
}

fn is_memo_match(tx1: &Transaction, tx2: &TransactionResponse) -> bool {
	if let Some(response_memo) = tx2.memo_text() {
		let Memo::MemoText(tx_memo) = &tx1.memo else {
			return false
		};

		if are_memos_eq(response_memo, tx_memo.get_vec()) {
			return true
		}
	}
	false
}

#[doc(hidden)]
/// A helper function which:
/// Returns true if the transaction in the middle of the list is a match;
/// Returns false if NOT a match;
/// Returns None if transaction can be found else where by narrowing down the search:
///  * if the sequence number of the transaction is > than what we're looking for, then update the
///    iterator by removing the first half of the list;
///  * else remove the last half of the list
///
/// # Arguments
///
/// * `iter` - the iterator to iterate over a list of `TransactionResponse`
/// * `tx` - the transaction we want to confirm if it's already submitted
/// * `public_key` - the public key of the wallet
fn _is_transaction_already_submitted(
	iter: &mut TransactionsResponseIter<Client>,
	tx: &Transaction,
	public_key: &PublicKey,
) -> Option<bool> {
	let tx_sequence_num = tx.seq_num;

	let Some(response) = iter.middle() else {
		return None
	};

	// Make sure that we are the sender and not the receiver because otherwise an
	// attacker could send a transaction to us with the target memo and we'd wrongly
	// assume that we already submitted this transaction.
	if !is_source_account_match(public_key, &response) {
		return None
	}

	let Ok(source_account_sequence)  = response.source_account_sequence() else {
		tracing::warn!("_is_transaction_already_submitted(): cannot extract sequence number of transaction response: {response:?}");
		return None
	};

	// check if the sequence number is the same as this response
	if tx_sequence_num == source_account_sequence {
		// Check if the transaction contains the memo we want to send
		return Some(is_memo_match(tx, &response))
	}

	// if the sequence number is greater than this response,
	// then the transaction must be in the first half of the list.
	if tx_sequence_num > source_account_sequence {
		iter.remove_last_half_records();
	}
	// the transaction must be in the last half of the list.
	else {
		iter.remove_first_half_records();
	}

	None
}

// handle tx_bad_seq
#[cfg_attr(test, mockable)]
impl StellarWallet {
	async fn handle_tx_bad_seq_error_with_xdr(
		&self,
		envelope_xdr_as_str_opt: &Option<String>,
	) -> Result<TransactionResponse, Error> {
		let tx_envelope = decode_to_envelope(envelope_xdr_as_str_opt)?;
		self.handle_tx_bad_seq_error_with_envelope(tx_envelope).await
	}

	async fn handle_tx_bad_seq_error_with_envelope(
		&self,
		tx_envelope: TransactionEnvelope,
	) -> Result<TransactionResponse, Error> {
		let tx = tx_envelope.get_transaction().ok_or(DecodeError)?;

		// Check if we already submitted this transaction
		if !self.is_transaction_already_submitted(&tx).await {
			// Remove original transaction.
			// The same envelope will be saved again using a different sequence number
			self.remove_tx_envelope_from_cache(&tx_envelope);

			return self.bump_sequence_number_and_submit(tx).await
		}

		tracing::error!("handle_tx_bad_seq_error_with_envelope(): Similar transaction already submitted. Skipping {:?}", tx);

		Err(ResubmissionError("Transaction already submitted".to_string()))
	}

	async fn bump_sequence_number_and_submit(
		&self,
		tx: Transaction,
	) -> Result<TransactionResponse, Error> {
		let sequence_number = self.get_sequence().await?;
		let mut updated_tx = tx.clone();
		updated_tx.seq_num = sequence_number + 1;

		let old_tx_xdr = tx.to_base64_xdr();
		let old_tx = String::from_utf8(old_tx_xdr.clone()).unwrap_or(format!("{old_tx_xdr:?}"));
		tracing::trace!("bump_sequence_number_and_submit(): old transaction: {old_tx}");

		let updated_tx_xdr = updated_tx.to_base64_xdr();
		let updated_tx_xdr =
			String::from_utf8(updated_tx_xdr.clone()).unwrap_or(format!("{updated_tx_xdr:?}"));
		tracing::trace!("bump_sequence_number_and_submit(): new transaction: {updated_tx_xdr}");

		let envelope = self.create_and_sign_envelope(updated_tx)?;
		self.submit_transaction(envelope).await
	}

	/// returns true if a transaction already exists and WAS submitted successfully.
	async fn is_transaction_already_submitted(&self, tx: &Transaction) -> bool {
		let tx_sequence_num = tx.seq_num;
		let default_page_size = SequenceNumber::from(DEFAULT_PAGE_SIZE);
		let own_public_key = self.public_key();

		// get the iterator
		let mut iter = match self.get_all_transactions_iter().await {
			Ok(iter) => iter,
			Err(e) => {
				tracing::warn!("is_transaction_already_submitted(): failed to get iterator: {e:?}");
				return false
			},
		};

		// iterate over the transactions
		while let Some(response) = iter.next().await {
			// Make sure that we are the sender and not the receiver because otherwise an
			// attacker could send a transaction to us with the target memo and we'd wrongly
			// assume that we already submitted this transaction.
			if !is_source_account_match(&own_public_key, &response) {
				// move to the next response
				continue
			}

			let Ok(source_account_sequence)  = response.source_account_sequence() else {
				tracing::warn!("is_transaction_already_submitted(): cannot extract sequence number of transaction response: {response:?}");
				// move to the next response
				continue
			};

			// check if the sequence number is the same as this response
			if tx_sequence_num == source_account_sequence {
				// Check if the transaction contains the memo we want to send
				return is_memo_match(tx, &response)
			}

			// if the sequence number is greater than this response,
			// no other transaction will ever match with it.
			if tx_sequence_num > source_account_sequence {
				break
			}

			// if the sequence number is greater than the last response of the iterator,
			// then the transaction is possibly in THIS page
			if tx_sequence_num >= source_account_sequence.saturating_sub(default_page_size) {
				// check the middle response OR remove half of the responses that won't match.
				if let Some(result) =
					_is_transaction_already_submitted(&mut iter, tx, &own_public_key)
				{
					// if the middle response matched (both source account and sequence number),
					// return that result
					return result
				}
				// if no match was found, move to the next response
				continue
			}

			// if the sequence number is lesser than the last response of the iterator,
			// then the transaction is possibly on the NEXT page
			if tx_sequence_num < source_account_sequence.saturating_sub(default_page_size) {
				if let None = iter.jump_to_next_page().await {
					// there's no pages left
					break
				}
			}
		}

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
	let Some(envelope_xdr) = envelope_xdr_as_str_opt else {
        tracing::warn!("handle_error(): no envelope_xdr found");
        return Err(ResubmissionError("no envelope_xdr".to_string()))
    };

	TransactionEnvelope::from_base64_xdr(envelope_xdr).map_err(|_| DecodeError)
}

#[cfg(test)]
mod test {
	use crate::{
		error::Error, mock::*, operations::create_basic_spacewalk_stellar_transaction,
		resubmissions::pause_process_in_secs, StellarWallet,
	};
	use mocktopus::mocking::{MockResult, Mockable};
	use primitives::{
		stellar::{types::AlphaNum4, Asset as StellarAsset, TransactionEnvelope, XdrCodec},
		TransactionEnvelopeExt,
	};
	use serial_test::serial;

	#[tokio::test]
	#[serial]
	async fn check_is_transaction_already_submitted() {
		let wallet = wallet_with_storage("resources/check_is_transaction_already_submitted")
			.expect("")
			.clone();
		let mut wallet = wallet.write().await;

		let asset = StellarAsset::native();
		let amount = 1002;

		// test is_transaction_already_submitted returns true
		{
			let response = wallet
				.send_payment_to_address(
					default_destination(),
					asset.clone(),
					amount,
					rand::random(),
					false,
				)
				.await
				.expect("should be ok");

			let tx_envelope = TransactionEnvelope::from_base64_xdr(response.envelope_xdr)
				.expect("should return an envelope");
			let tx = tx_envelope.get_transaction().expect("should return a transaction");

			// check that the transaction truly exists
			assert!(wallet.is_transaction_already_submitted(&tx).await);
		}

		// test is_transaction_already_submitted when transaction is on the next pages
		{
			// sequence number: 1017907249295
			let envelope = "AAAAAgAAAACI3DQX1QWOxLRQPgwS6hoKib4gD+mJIkI9QzQBT6aw7gAAAGQAAADtAAAAjwAAAAAAAAABAAAAHDExMTExMTExMTExMTExMTExMTExMTExMTExMTEAAAABAAAAAQAAAACI3DQX1QWOxLRQPgwS6hoKib4gD+mJIkI9QzQBT6aw7gAAAAEAAAAAO5kROA7+mIugqJAOsc/kTzZvfb6Ua+0HckD39iTfFcUAAAAAAAAAAAAAA+gAAAAAAAAAAU+msO4AAABAMVQZONg4CsTjVe7nmrY2LX86a7VWrmv8uL37zkqwY9Qpxte/76pUnZ/hN8o7EkpzBAWr5qb85cvzAlPgbQVGCA==";
			let tx_env = TransactionEnvelope::from_base64_xdr(envelope)
				.expect("should decode into transactionenvelope");
			let tx = tx_env.get_transaction().expect("should return a transaction");

			assert!(wallet.is_transaction_already_submitted(&tx).await);
		}

		// test is_transaction_already_submitted returns false
		{
			let dummy_tx = create_basic_spacewalk_stellar_transaction(
				rand::random(),
				DEFAULT_STROOP_FEE_PER_OPERATION,
				wallet.public_key(),
				1,
			)
			.expect("return a transaction");

			assert!(!wallet.is_transaction_already_submitted(&dummy_tx).await);
		}

		wallet.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn check_bump_sequence_number_and_submit() {
		let wallet = wallet_with_storage("resources/check_bump_sequence_number_and_submit")
			.expect("should return a wallet")
			.clone();
		let wallet = wallet.write().await;
		let seq = wallet.get_sequence().await.expect("return sequence number");

		let asset = StellarAsset::native();
		let amount = 1002;

		// test bump_sequence_number_and_submit success
		{
			let dummy_envelope = wallet
				.create_payment_envelope(
					default_destination(),
					asset.clone(),
					amount,
					rand::random(),
					DEFAULT_STROOP_FEE_PER_OPERATION,
					seq - 5,
				)
				.expect("should return an envelope");

			let dummy_transaction =
				dummy_envelope.get_transaction().expect("must return a transaction");

			let resp = wallet
				.bump_sequence_number_and_submit(dummy_transaction.clone())
				.await
				.expect("return ok");
			let new_dummy_transaction =
				String::from_utf8(resp.envelope_xdr).expect("should return a String");
			let new_dummy_env = TransactionEnvelope::from_base64_xdr(new_dummy_transaction)
				.expect("should return an envelope");
			let new_dummy_transaction =
				new_dummy_env.get_transaction().expect("should return a transaction");

			assert!(wallet.is_transaction_already_submitted(&new_dummy_transaction).await);
		}

		// test bump_sequence_number_and_submit failed
		{
			let dummy_envelope = wallet
				.create_payment_envelope(
					default_destination(),
					asset,
					amount,
					rand::random(),
					DEFAULT_STROOP_FEE_PER_OPERATION,
					seq,
				)
				.expect("should return an envelope");
			let dummy_tx = dummy_envelope.get_transaction().expect("should return a tx");

			StellarWallet::sign_envelope
				.mock_safe(move |_, _| MockResult::Return(Err(Error::SignEnvelopeError)));

			match wallet.bump_sequence_number_and_submit(dummy_tx).await {
				Err(Error::SignEnvelopeError) => assert!(true),
				other => panic!("expecting Error::SignEnvelopeError, found: {other:?}"),
			};
		}

		wallet.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn check_handle_tx_bad_seq_error_with_envelope() {
		let wallet = wallet_with_storage("resources/check_handle_tx_bad_seq_error_with_envelope")
			.expect("should return a wallet")
			.clone();
		let wallet = wallet.write().await;

		let dummy_envelope = wallet
			.create_dummy_envelope_no_signature(1003)
			.await
			.expect("should return an envelope");

		// test handle_tx_bad_seq_error_with_envelope is success
		{
			StellarWallet::is_transaction_already_submitted
				.mock_safe(move |_, _| MockResult::Return(Box::pin(async move { false })));

			assert!(wallet
				.handle_tx_bad_seq_error_with_envelope(dummy_envelope.clone())
				.await
				.is_ok());
		}

		// test handle_tx_bad_seq_error_with_envelope is fail
		{
			StellarWallet::is_transaction_already_submitted
				.mock_safe(move |_, _| MockResult::Return(Box::pin(async move { true })));

			match wallet.handle_tx_bad_seq_error_with_envelope(dummy_envelope).await {
				Err(Error::ResubmissionError(_)) => assert!(true),
				other => panic!("expecting Error::ResubmissionError, found: {other:?}"),
			}
		}

		wallet.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn check_handle_tx_internal_error() {
		let wallet = wallet_with_storage("resources/check_handle_tx_internal_error")
			.expect("should return a wallet")
			.clone();
		let wallet = wallet.write().await;
		let sequence = wallet.get_sequence().await.expect("return a sequence");

		let envelope = wallet
			.create_payment_envelope_no_signature(
				default_destination(),
				StellarAsset::native(),
				100,
				rand::random(),
				DEFAULT_STROOP_FEE_PER_OPERATION,
				sequence + 1,
			)
			.expect("should return an envelope");

		let envelope_xdr = envelope.to_base64_xdr();
		// Convert vec to string (because the HorizonSubmissionError always returns a string)
		let envelope_xdr =
			Some(String::from_utf8(envelope_xdr).expect("should create string from vec"));

		// test handle_tx_internal_error is success
		if let Err(e) = wallet.handle_tx_internal_error(&envelope_xdr).await {
			panic!("expect a success, found error: {e:?}");
		};

		// test handle_tx_internal_error is fail
		{
			StellarWallet::submit_transaction.mock_safe(move |_, _| {
				MockResult::Return(Box::pin(async move { Err(Error::DecodeError) }))
			});

			match wallet.handle_tx_internal_error(&envelope_xdr).await {
				Err(Error::DecodeError) => assert!(true),
				other => {
					panic!("expect an Error::DecodeError, found: {other:?}");
				},
			}
		}

		wallet.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn check_handle_error() {
		let wallet = wallet_with_storage("resources/check_handle_error")
			.expect("should return a wallet")
			.clone();
		let wallet = wallet.write().await;

		// tx_bad_seq test
		{
			let envelope = wallet
				.create_dummy_envelope_no_signature(1010)
				.await
				.expect("returns an envelope");
			let envelope_xdr = envelope.to_base64_xdr();
			let envelope_xdr = String::from_utf8(envelope_xdr).ok();

			// result is success
			{
				let error = Error::HorizonSubmissionError {
					title: "title".to_string(),
					status: 400,
					reason: "tx_bad_seq".to_string(),
					result_code_op: vec![],
					envelope_xdr,
				};

				StellarWallet::is_transaction_already_submitted
					.mock_safe(move |_, _| MockResult::Return(Box::pin(async move { false })));

				assert!(wallet.handle_error(error).await.is_ok());
			}

			// result is error
			{
				let error = Error::HorizonSubmissionError {
					title: "title".to_string(),
					status: 400,
					reason: "tx_bad_seq".to_string(),
					result_code_op: vec![],
					envelope_xdr: None,
				};

				match wallet.handle_error(error).await {
					Err(Error::ResubmissionError(_)) => assert!(true),
					other => panic!("expecting Error::ResubmissionError, found: {other:?}"),
				};
			}
		}

		// tx_internal_error test
		{
			let envelope = wallet
				.create_dummy_envelope_no_signature(1020)
				.await
				.expect("returns an envelope");
			let envelope_xdr = envelope.to_base64_xdr();
			let envelope_xdr = String::from_utf8(envelope_xdr).ok();

			// result is success
			{
				let error = Error::HorizonSubmissionError {
					title: "title".to_string(),
					status: 400,
					reason: "tx_internal_error".to_string(),
					result_code_op: vec![],
					envelope_xdr,
				};

				assert!(wallet.handle_error(error).await.is_ok());
			}

			// result is error
			{
				let error = Error::HorizonSubmissionError {
					title: "title".to_string(),
					status: 400,
					reason: "tx_internal_error".to_string(),
					result_code_op: vec![],
					envelope_xdr: None,
				};

				match wallet.handle_error(error).await {
					Err(Error::ResubmissionError(_)) => assert!(true),
					other => panic!("expecting Error::ResubmissionError, found: {other:?}"),
				};
			}
		}

		// other error
		{
			let envelope = wallet
				.create_dummy_envelope_no_signature(1010)
				.await
				.expect("returns an envelope");
			let envelope_xdr = envelope.to_base64_xdr();
			let envelope_xdr = String::from_utf8(envelope_xdr).ok();

			let error = Error::HorizonSubmissionError {
				title: "Transaction Failed".to_string(),
				status: 400,
				reason: "tx_bad_auth".to_string(),
				result_code_op: vec![],
				envelope_xdr,
			};

			match wallet.handle_error(error).await {
				// `tx_bad_auth` is not recoverable so we expect `Ok(None)`
				Ok(None) => assert!(true),
				other => panic!("expect an Ok(None), found: {other:?}"),
			}
		}

		wallet.remove_cache_dir();
	}

	#[tokio::test]
	#[serial]
	async fn resubmit_transactions_works() {
		let wallet = wallet_with_storage("resources/resubmit_transactions_works")
			.expect("should return a wallet")
			.clone();
		let wallet = wallet.write().await;

		let seq_number = wallet.get_sequence().await.expect("should return a sequence");

		let mut asset_code: [u8; 4] = [0; 4];
		asset_code.copy_from_slice("EURO".as_bytes());

		// creating a bad envelope
		let non_recoverable_envelope = wallet
			.create_payment_envelope_no_signature(
				wallet.public_key(),
				StellarAsset::AssetTypeCreditAlphanum4(AlphaNum4 {
					asset_code,
					issuer: default_destination(),
				}),
				1001,
				rand::random(),
				DEFAULT_STROOP_FEE_PER_OPERATION,
				seq_number + 2,
			)
			.expect("should return an envelope");

		// let's save this in storage
		let _ = wallet
			.save_tx_envelope_to_cache(non_recoverable_envelope.clone())
			.expect("should save.");

		// creating a bad (but recoverable) envelope
		let recoverable_envelope = wallet
			.create_payment_envelope(
				default_destination(),
				StellarAsset::native(),
				1100,
				rand::random(),
				DEFAULT_STROOP_FEE_PER_OPERATION,
				seq_number + 10,
			)
			.expect("should return an envelope");

		// let's save this in storage
		let _ = wallet
			.save_tx_envelope_to_cache(recoverable_envelope.clone())
			.expect("should save.");

		// create a good envelope
		let good_envelope = wallet
			.create_payment_envelope(
				default_destination(),
				StellarAsset::native(),
				1002,
				rand::random(),
				DEFAULT_STROOP_FEE_PER_OPERATION,
				seq_number + 1,
			)
			.expect("should return an envelope");

		// let's save this in storage
		let _ = wallet.save_tx_envelope_to_cache(good_envelope.clone()).expect("should save.");

		StellarWallet::is_transaction_already_submitted
			.mock_safe(move |_, _| MockResult::Return(Box::pin(async move { false })));

		// let's resubmit these 3 transactions
		let _ = wallet.start_periodic_resubmission_of_transactions_from_cache(60).await;

		// We wait until the whole cache is empty because eventually all transactions should be
		// handled
		pause_process_in_secs(10).await;

		loop {
			let (txs, _) = wallet.get_tx_envelopes_from_cache().expect("return a tuple");

			if txs.is_empty() {
				assert_eq!(
					wallet.get_sequence().await.expect("should return a sequence"),
					seq_number + 2
				);
				break
			}
		}

		wallet.remove_cache_dir();
	}
}
