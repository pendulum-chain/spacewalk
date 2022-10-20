use stellar_relay::sdk::{
	types::{PaymentOp, TransactionSet},
	Transaction, TransactionEnvelope,
};

use crate::oracle::{
	collector::{is_hash_memo, Proof, ProofStatus, ScpMessageCollector},
	errors::Error,
	types::Slot,
	TxFilterMap,
};

impl ScpMessageCollector {
	/// Returns a list of transactions (only those with proofs) to be processed.
	pub fn get_pending_proofs(&mut self) -> Vec<Proof> {
		// Store the handled transaction indices in a vec to be able to remove them later
		let mut handled_tx_indices = Vec::new();

		let mut proofs_for_handled_txs = Vec::<Proof>::new();

		for (index, (tx_env, slot)) in self.pending_transactions.iter().enumerate() {
			// Try to build proofs
			match self.build_proof(tx_env.clone(), *slot) {
				ProofStatus::Proof(proof) => {
					handled_tx_indices.push(index);
					proofs_for_handled_txs.push(proof);
				},
				_ => {},
			}
		}
		// Remove the transactions with proofs from the pending list.
		for index in handled_tx_indices.iter().rev() {
			self.pending_transactions.remove(*index);
		}

		proofs_for_handled_txs
	}
}

// Checking for Tx Relevance
impl ScpMessageCollector {
	/// Checks whether the transaction is relevant to this vault.
	pub(crate) fn is_tx_relevant(&self, transaction_env: &TransactionEnvelope) -> bool {
		match transaction_env {
			TransactionEnvelope::EnvelopeTypeTx(value) =>
				self._is_tx_relevant(&value.tx) && is_hash_memo(&value.tx.memo),
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
