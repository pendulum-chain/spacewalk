use crate::oracle::{
	collector::{check_memo, EncodedProof, ProofStatus, ScpMessageCollector},
	errors::Error,
	types::Slot,
	TxFilterMap,
};
use stellar_relay::sdk::{
	types::{PaymentOp, TransactionSet},
	Transaction, TransactionEnvelope,
};

impl ScpMessageCollector {
	/// maps the slot to the transactions of the TransactionSet
	///
	/// # Arguments
	///
	/// * `slot` - the slot of which the tx belongs to
	/// * `tx_set` - where the txs are derived from.
	/// * `filter` - filters out transactions (in the Transaction Set) for processing.
	pub(super) fn update_tx_hash_map(
		&mut self,
		slot: Slot,
		tx_set: &TransactionSet,
		filters: &TxFilterMap,
	) -> Result<(), Error> {
		tracing::info!("Inserting received transaction set for slot {}", slot);

		// Collect tx hashes to save to file.
		tx_set.txes.get_vec().iter().for_each(|tx_env| {
			let tx_hash = tx_env.get_hash(self.network());

			// only relevant transactions are saved.
			if self.is_tx_relevant(tx_env) {
				tracing::info!("saving to {:?} to hash map", base64::encode(&tx_hash));
				self.tx_hash_map_mut().insert(tx_hash, slot);
			}

			// loops through the filters to check if transaction needs to be processed.
			// Add transaction to pending transactions if it is not yet contained
			while let Some(filter) = filters.values().next() {
				if filter.check_for_processing(tx_env) {
					if self
						.pending_transactions
						.iter()
						.find(|(tx, _)| tx.get_hash(self.network()) == tx_hash)
						.is_none()
					{
						self.pending_transactions.push((tx_env.clone(), slot));
						break
					}
				}
			}
		});

		Ok(())
	}

	/// Returns a list of transactions (only those with proofs) to be processed.
	pub fn get_pending_txs(&mut self) -> Vec<EncodedProof> {
		// Store the handled transaction indices in a vec to be able to remove them later
		let mut handled_tx_indices = Vec::new();

		let mut handled_txs = Vec::new();

		for (index, (tx_env, slot)) in self.pending_transactions.iter().enumerate() {
			// Try to build proofs
			match self.build_proof(tx_env.clone(), *slot) {
				ProofStatus::Proof(proof) => {
					handled_tx_indices.push(index);
					handled_txs.push(proof);
				},
				_ => {},
			}
		}
		// Remove the transactions with proofs from the pending list.
		for index in handled_tx_indices.iter().rev() {
			self.pending_transactions.remove(*index);
		}

		handled_txs
	}
}

// Checking for Tx Relevance
impl ScpMessageCollector {
	/// Checks whether the transaction is relevant to this vault.
	fn is_tx_relevant(&self, transaction_env: &TransactionEnvelope) -> bool {
		match transaction_env {
			TransactionEnvelope::EnvelopeTypeTx(value) =>
				self._is_tx_relevant(&value.tx) && check_memo(&value.tx.memo),
			_ => false,
		}
	}

	/// helper method of `is_tx_relevant()`
	fn _is_tx_relevant(&self, transaction: &Transaction) -> bool {
		let payment_ops_to_vault_address: Vec<&PaymentOp> = transaction
			.operations
			.get_vec()
			.into_iter()
			.filter_map(|op| match &op.body {
				stellar_relay::sdk::types::OperationBody::Payment(p) => {
					let d = p.destination.clone();
					if self.vault_addresses.contains(
						&String::from_utf8(Vec::from(d.to_encoding().as_slice())).unwrap(),
					) {
						Some(p)
					} else {
						None
					}
				},
				_ => None,
			})
			.collect();

		if payment_ops_to_vault_address.len() == 0 {
			// The transaction is not relevant to use since it doesn't
			// include a payment to our vault address
			return false
		} else {
			tracing::info!("Transaction to our vault address received.");
			let source = transaction.source_account.clone();
			for payment_op in payment_ops_to_vault_address {
				let destination = payment_op.destination.clone();
				// let amount = payment_op.amount;
				// let asset = payment_op.asset.clone();
				// tracing::info!("Deposit amount {:?} stroops", amount);
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
			return true
		}
	}
}
