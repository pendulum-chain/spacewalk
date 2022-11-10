//! A collection of node-specific RPC methods.
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

#![warn(missing_docs)]

pub use jsonrpsee;
use primitives::{
	issue::IssueRequest, AccountId, Balance, Block, BlockNumber, CurrencyId, Nonce, VaultId,
};
pub use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_arithmetic::FixedU128;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_core::H256;
use std::sync::Arc;

/// Full client dependencies.
pub struct FullDeps<C, P> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
}

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpsee::RpcModule<()>;

/// Instantiate all full RPC extensions.
pub fn create_full<C, P>(
	deps: FullDeps<C, P>,
) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + 'static,
	C: Send + Sync + 'static,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: module_issue_rpc::IssueRuntimeApi<
		Block,
		AccountId,
		H256,
		IssueRequest<AccountId, BlockNumber, Balance, CurrencyId>,
	>,
	C::Api: module_vault_registry_rpc::VaultRegistryRuntimeApi<
		Block,
		VaultId<AccountId, CurrencyId>,
		Balance,
		FixedU128,
		CurrencyId,
		AccountId,
	>,
	C::Api: BlockBuilder<Block>,
	P: TransactionPool + 'static,
{
	use module_issue_rpc::{Issue, IssueApiServer};
	use module_vault_registry_rpc::{VaultRegistry, VaultRegistryApiServer};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
	use substrate_frame_rpc_system::{System, SystemApiServer};

	let mut module = RpcExtension::new(());
	let FullDeps { client, pool, deny_unsafe } = deps;
	module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;

	module.merge(TransactionPayment::new(client.clone()).into_rpc())?;

	module.merge(VaultRegistry::new(client.clone()).into_rpc())?;

	module.merge(Issue::new(client.clone()).into_rpc())?;

	Ok(module)
}
