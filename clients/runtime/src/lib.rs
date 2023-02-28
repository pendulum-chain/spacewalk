use codec::{Decode, Encode};
pub use prometheus;
pub use sp_arithmetic::{traits as FixedPointTraits, FixedI128, FixedPointNumber, FixedU128};
use sp_std::marker::PhantomData;
pub use subxt::{
	events::StaticEvent,
	ext::sp_core::{crypto::Ss58Codec, sr25519::Pair},
};
use subxt::{
	ext::sp_runtime::{generic::Header, traits::BlakeTwo256, MultiSignature},
	subxt, Config,
};

pub use assets::TryFromSymbol;
pub use error::{Error, SubxtError};
pub use retry::{notify_retry, RetryPolicy};
pub use rpc::{
	CollateralBalancesPallet, IssuePallet, OraclePallet, RedeemPallet, ReplacePallet,
	SecurityPallet, SpacewalkParachain, StellarRelayPallet, SudoPallet, UtilFuncs,
	VaultRegistryPallet, DEFAULT_SPEC_NAME, SS58_PREFIX,
};
pub use shutdown::{ShutdownReceiver, ShutdownSender};
pub use types::*;

pub mod cli;

#[cfg(test)]
mod tests;

#[cfg(feature = "testing-utils")]
pub mod integration;

mod assets;
mod conn;
mod error;
mod retry;
mod rpc;
mod shutdown;
pub mod types;

pub const TX_FEES: u128 = 2000000000;
pub const MILLISECS_PER_BLOCK: u64 = 6000;

pub const STELLAR_RELAY_MODULE: &str = "StellarRelay";
pub const ISSUE_MODULE: &str = "Issue";
pub const REDEEM_MODULE: &str = "Redeem";
pub const SECURITY_MODULE: &str = "Security";
pub const SYSTEM_MODULE: &str = "System";

pub const STABLE_PARACHAIN_CONFIRMATIONS: &str = "StableParachainConfirmations";

// All of the parachain features use the same metadata (from Foucoco) for now.
// We can change this once the spacewalk pallets were added to the runtimes of the other chains as well.
#[cfg_attr(
	feature = "parachain-metadata-pendulum",
	subxt(
		runtime_metadata_path = "metadata-parachain-foucoco.scale",
		derive_for_all_types = "Clone, PartialEq, Eq",
	)
)]
#[cfg_attr(
	feature = "parachain-metadata-amplitude",
	subxt(
		runtime_metadata_path = "metadata-parachain-foucoco.scale",
		derive_for_all_types = "Clone, PartialEq, Eq",
	)
)]
#[cfg_attr(
	feature = "parachain-metadata-foucoco",
	subxt(
		runtime_metadata_path = "metadata-parachain-foucoco.scale",
		derive_for_all_types = "Clone, PartialEq, Eq",
	)
)]
#[cfg_attr(
	feature = "standalone-metadata",
	subxt(
		runtime_metadata_path = "metadata-standalone.scale",
		derive_for_all_types = "Clone, PartialEq, Eq",
	)
)]
pub mod metadata {
	#[subxt(substitute_type = "sp_core::crypto::AccountId32")]
	use crate::AccountId;
	#[subxt(substitute_type = "spacewalk_primitives::CurrencyId")]
	use crate::CurrencyId;
	#[subxt(substitute_type = "sp_arithmetic::fixed_point::FixedU128")]
	use crate::FixedU128;
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Default, Clone, Decode, Encode)]
pub struct WrapperKeepOpaque<T> {
	data: Vec<u8>,
	_phantom: PhantomData<T>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SpacewalkRuntime;

impl Config for SpacewalkRuntime {
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Address = Address;
	type Header = Header<Self::BlockNumber, BlakeTwo256>;
	type Signature = MultiSignature;
	type ExtrinsicParams = subxt::tx::PolkadotExtrinsicParams<Self>;
}
