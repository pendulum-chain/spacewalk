use std::{sync::Arc, time::Duration};

use runtime::{CurrencyId, ShutdownSender, SpacewalkParachain, StellarPublicKey, VaultId, H256};
use service::{spawn_cancelable, Error as ServiceError};
use wallet::StellarWallet;

use crate::{error::Error, VaultIdManager};

#[derive(Debug, Clone, PartialEq)]
struct Deadline {
	parachain: u32,
	bitcoin: u32,
}

#[derive(Debug, Clone)]
pub struct Request {
	hash: H256,
	/// Deadline (unit: active block number) after which payments will no longer be attempted.
	deadline: Option<Deadline>,
	amount: u128,
	asset: CurrencyId,
	stellar_address: StellarPublicKey,
	request_type: RequestType,
	vault_id: VaultId,
	fee_budget: Option<u128>,
}

#[derive(Debug, Copy, Clone)]
pub enum RequestType {
	Redeem,
	Replace,
}

impl Request {
	// TODO
}

/// Queries the parachain for open requests and executes them. It checks the
/// stellar blockchain to see if a payment has already been made.
#[allow(clippy::too_many_arguments)]
pub async fn execute_open_requests(
	shutdown_tx: ShutdownSender,
	parachain_rpc: SpacewalkParachain,
	vault_id_manager: VaultIdManager,
	read_only_stellar_wallet: Arc<StellarWallet>,
	payment_margin: Duration,
) -> Result<(), ServiceError<Error>> {
	// TODO
	Ok(())
}
