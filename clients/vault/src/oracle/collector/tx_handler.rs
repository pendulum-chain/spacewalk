//todo: to be deleted, once this logic is transferred

use stellar_relay::sdk::{types::PaymentOp, Transaction, TransactionEnvelope};

use crate::oracle::collector::{is_hash_memo, Proof, ProofStatus, ScpMessageCollector};
use crate::oracle::types::Slot;

impl ScpMessageCollector {


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
