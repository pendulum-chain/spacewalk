use std::{collections::HashMap, convert::TryInto, time::Duration};

use futures::{future::Either, stream::StreamExt, try_join, TryStreamExt};
use governor::RateLimiter;
use tokio::time::sleep;
use tokio_stream::wrappers::BroadcastStream;

use runtime::{
	CurrencyId, Error as RuntimeError, FixedPointNumber, FixedU128, OraclePallet, PrettyPrint,
	SpacewalkParachain, StellarPublicKey, UtilFuncs, VaultId, VaultRegistryPallet, H256,
};
use service::{spawn_cancelable, DynBitcoinCoreApi, Error as ServiceError, ShutdownSender};

use crate::{
	error::Error,
	metrics::update_bitcoin_metrics,
	stellar_wallet::StellarWallet,
	system::{VaultData, VaultIdManager},
	VaultIdManager, YIELD_RATE,
};

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
	read_only_stellar_wallet: StellarWallet,
	payment_margin: Duration,
) -> Result<(), ServiceError<Error>> {
	// TODO
	Ok(())
}
