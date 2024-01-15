//! A collection of node-specific RPC methods.
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

#![warn(missing_docs)]

use std::sync::Arc;

use jsonrpc_core::futures;
pub use jsonrpsee;
use sc_consensus_manual_seal::rpc::{EngineCommand, ManualSeal, ManualSealApiServer};
pub use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool_api::TransactionPool;
use sp_blockchain::HeaderMetadata;

use primitives::Hash;

use crate::service::{FullMainnetClient, FullTestnetClient};

/// Full client dependencies.
pub struct FullDeps<C, P> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
	/// Manual seal command sink
	pub command_sink: Option<futures::channel::mpsc::Sender<EngineCommand<Hash>>>,
}

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpsee::RpcModule<()>;
pub type ResultRpcExtension = Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>;

pub fn create_full_mainnet<P>(deps: FullDeps<FullMainnetClient, P>) -> ResultRpcExtension
where
	P: TransactionPool + Sync + Send + 'static,
{
	use module_issue_rpc::{Issue, IssueApiServer};
	use module_oracle_rpc::{Oracle, OracleApiServer};
	use module_redeem_rpc::{Redeem, RedeemApiServer};
	use module_replace_rpc::{Replace, ReplaceApiServer};
	use module_vault_registry_rpc::{VaultRegistry, VaultRegistryApiServer};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
	use substrate_frame_rpc_system::{System, SystemApiServer};

	let mut module = RpcExtension::new(());
	let FullDeps { client, pool, deny_unsafe, command_sink } = deps;

	if let Some(command_sink) = command_sink {
		module.merge(
			// We provide the rpc handler with the sending end of the channel to allow the rpc
			// send EngineCommands to the background block authorship task.
			ManualSeal::new(command_sink).into_rpc(),
		)?;
	}

	module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
	module.merge(TransactionPayment::new(client.clone()).into_rpc())?;
	module.merge(Issue::new(client.clone()).into_rpc())?;
	module.merge(Redeem::new(client.clone()).into_rpc())?;
	module.merge(Replace::new(client.clone()).into_rpc())?;
	module.merge(Oracle::new(client.clone()).into_rpc())?;
	module.merge(VaultRegistry::new(client).into_rpc())?;

	Ok(module)
}

pub fn create_full_testnet<P>(deps: FullDeps<FullTestnetClient, P>) -> ResultRpcExtension
where
	P: TransactionPool + Sync + Send + 'static,
{
	use module_issue_rpc::{Issue, IssueApiServer};
	use module_oracle_rpc::{Oracle, OracleApiServer};
	use module_redeem_rpc::{Redeem, RedeemApiServer};
	use module_replace_rpc::{Replace, ReplaceApiServer};
	use module_vault_registry_rpc::{VaultRegistry, VaultRegistryApiServer};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
	use substrate_frame_rpc_system::{System, SystemApiServer};

	let mut module = RpcExtension::new(());
	let FullDeps { client, pool, deny_unsafe, command_sink } = deps;

	if let Some(command_sink) = command_sink {
		module.merge(
			// We provide the rpc handler with the sending end of the channel to allow the rpc
			// send EngineCommands to the background block authorship task.
			ManualSeal::new(command_sink).into_rpc(),
		)?;
	}

	module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
	module.merge(TransactionPayment::new(client.clone()).into_rpc())?;
	module.merge(Issue::new(client.clone()).into_rpc())?;
	module.merge(Redeem::new(client.clone()).into_rpc())?;
	module.merge(Replace::new(client.clone()).into_rpc())?;
	module.merge(Oracle::new(client.clone()).into_rpc())?;
	module.merge(VaultRegistry::new(client).into_rpc())?;

	Ok(module)
}
