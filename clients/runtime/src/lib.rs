use codec::{Decode, Encode};
pub use sp_arithmetic::{traits as FixedPointTraits, FixedI128, FixedPointNumber, FixedU128};
use sp_std::marker::PhantomData;
pub use subxt::ext::sp_core::{crypto::Ss58Codec, sr25519::Pair};
use subxt::{
	ext::sp_runtime::{generic::Header, traits::BlakeTwo256, MultiSignature, OpaqueExtrinsic},
	subxt, Config,
};

pub use assets::TryFromSymbol;
pub use error::{Error, SubxtError};
pub use prometheus;
pub use retry::{notify_retry, RetryPolicy};
pub use rpc::{
	CollateralBalancesPallet, SpacewalkParachain, UtilFuncs, VaultRegistryPallet,
	DEFAULT_SPEC_NAME, SS58_PREFIX,
};
pub use types::*;

pub mod cli;

mod assets;
mod conn;
mod error;
mod retry;
mod rpc;

pub mod types;

pub const TX_FEES: u128 = 2000000000;
pub const MILLISECS_PER_BLOCK: u64 = 6000;

pub const BTC_RELAY_MODULE: &str = "BTCRelay";
pub const ISSUE_MODULE: &str = "Issue";
pub const REDEEM_MODULE: &str = "Redeem";
pub const RELAY_MODULE: &str = "Relay";
pub const SECURITY_MODULE: &str = "Security";
pub const SYSTEM_MODULE: &str = "System";

pub const STABLE_BITCOIN_CONFIRMATIONS: &str = "StableBitcoinConfirmations";
pub const STABLE_PARACHAIN_CONFIRMATIONS: &str = "StableParachainConfirmations";

#[cfg_attr(
	feature = "parachain-metadata",
	subxt(
		runtime_metadata_path = "metadata-parachain.scale",
		derive_for_all_types = "Clone, PartialEq, Eq",
		derive_for_type(
			type = "spacewalk_primitives::TokenSymbol",
			derive = "serde::Serialize, serde::Deserialize"
		),
		derive_for_type(
			type = "spacewalk_primitives::CurrencyId",
			derive = "serde::Serialize, serde::Deserialize"
		),
		derive_for_type(
			type = "spacewalk_primitives::VaultCurrencyPair",
			derive = "serde::Serialize, serde::Deserialize"
		),
		derive_for_type(
			type = "spacewalk_primitives::VaultId",
			derive = "serde::Serialize, serde::Deserialize"
		),
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
	// TODO why does it fix it? What does it do?
	#[subxt(substitute_type = "spacewalk_primitives::CurrencyId")]
	use crate::CurrencyId;
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Default, Clone, Decode, Encode)]
pub struct WrapperKeepOpaque<T> {
	data: Vec<u8>,
	_phantom: PhantomData<T>,
}

type SpacewalkRuntime = subxt::PolkadotConfig; // TODO check if this should be substrate or polkadot config

// #[derive(Debug, Clone, Eq, PartialEq)]
// pub struct SpacewalkRuntime;
//
// impl Config for SpacewalkRuntime {
// 	type Index = Index;
// 	type BlockNumber = BlockNumber;
// 	type Hash = H256;
// 	type Hashing = BlakeTwo256;
// 	type AccountId = AccountId;
// 	type Address = Address;
// 	type Header = Header<Self::BlockNumber, BlakeTwo256>;
// 	type Signature = MultiSignature;
// 	type Extrinsic = OpaqueExtrinsic;
// 	type ExtrinsicParams = (); // TODO figure this out
// }
