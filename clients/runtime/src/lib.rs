pub use assets::TryFromSymbol;
use codec::{Decode, Encode};
pub use error::{Error, Recoverability, SubxtError};
pub use primitives::CurrencyInfo;
pub use prometheus;
pub use retry::{notify_retry, RetryPolicy};
#[cfg(feature = "testing-utils")]
pub use rpc::SudoPallet;
pub use rpc::{
	CollateralBalancesPallet, IssuePallet, OraclePallet, RedeemPallet, ReplacePallet,
	SecurityPallet, SpacewalkParachain, StellarRelayPallet, UtilFuncs, VaultRegistryPallet,
	DEFAULT_SPEC_NAME, SS58_PREFIX,
};
pub use shutdown::{ShutdownReceiver, ShutdownSender};
pub use sp_arithmetic::{traits as FixedPointTraits, FixedI128, FixedPointNumber, FixedU128};
use sp_std::marker::PhantomData;
use subxt::{
	client::OfflineClientT, config::polkadot::PolkadotExtrinsicParams,
	ext::sp_runtime::MultiSignature, subxt, Config,
};
pub use subxt::{
	config::substrate::BlakeTwo256,
	events::StaticEvent,
	ext::sp_core::{crypto::Ss58Codec, sr25519::Pair},
};
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

// Sanity check: Make sure that at least one feature is selected.
#[cfg(not(any(
	feature = "standalone-metadata",
	feature = "parachain-metadata-foucoco",
	feature = "parachain-metadata-pendulum",
	feature = "parachain-metadata-amplitude"
)))]
compile_error!("You need to select at least one of the metadata features");

// If all features are selected, then we need to select only one metadata feature.
// Since this is done for testing, we can select the standalone metadata.
#[cfg_attr(
	feature = "standalone-metadata",
	subxt(
		runtime_metadata_path = "metadata-standalone.scale",
		derive_for_all_types = "Clone, PartialEq, Eq",
		substitute_type(path = "sp_core::crypto::AccountId32", with = "crate::AccountId"),
		substitute_type(
			path = "spacewalk_primitives::CurrencyId",
			with = "::subxt::utils::Static<crate::CurrencyId>"
		),
		substitute_type(
			path = "sp_arithmetic::fixed_point::FixedU128",
			with = "::subxt::utils::Static<crate::FixedU128>"
		),
	)
)]
#[cfg_attr(
	all(feature = "parachain-metadata-pendulum", not(feature = "standalone-metadata")),
	subxt(
		runtime_metadata_path = "metadata-parachain-pendulum.scale",
		derive_for_all_types = "Clone, PartialEq, Eq",
		substitute_type(path = "sp_core::crypto::AccountId32", with = "crate::AccountId"),
		substitute_type(
			path = "spacewalk_primitives::CurrencyId",
			with = "::subxt::utils::Static<crate::CurrencyId>"
		),
		substitute_type(
			path = "sp_arithmetic::fixed_point::FixedU128",
			with = "::subxt::utils::Static<crate::FixedU128>"
		),
	)
)]
#[cfg_attr(
	all(feature = "parachain-metadata-amplitude", not(feature = "standalone-metadata")),
	subxt(
		runtime_metadata_path = "metadata-parachain-amplitude.scale",
		derive_for_all_types = "Clone, PartialEq, Eq",
		substitute_type(path = "sp_core::crypto::AccountId32", with = "crate::AccountId"),
		substitute_type(
			path = "spacewalk_primitives::CurrencyId",
			with = "::subxt::utils::Static<crate::CurrencyId>"
		),
		substitute_type(
			path = "sp_arithmetic::fixed_point::FixedU128",
			with = "::subxt::utils::Static<crate::FixedU128>"
		),
	)
)]
#[cfg_attr(
	all(feature = "parachain-metadata-foucoco", not(feature = "standalone-metadata")),
	subxt(
		runtime_metadata_path = "metadata-parachain-foucoco.scale",
		derive_for_all_types = "Clone, PartialEq, Eq",
		substitute_type(path = "sp_core::crypto::AccountId32", with = "crate::AccountId"),
		substitute_type(
			path = "spacewalk_primitives::CurrencyId",
			with = "::subxt::utils::Static<crate::CurrencyId>"
		),
		substitute_type(
			path = "sp_arithmetic::fixed_point::FixedU128",
			with = "::subxt::utils::Static<crate::FixedU128>"
		),
	)
)]
pub mod metadata {}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Default, Clone, Decode, Encode)]
pub struct WrapperKeepOpaque<T> {
	data: Vec<u8>,
	_phantom: PhantomData<T>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SpacewalkRuntime;

use subxt::config::{signed_extensions, ExtrinsicParamsError};

pub type CustomExtrinsicParams<T> = signed_extensions::AnyOf<
	T,
	(
		signed_extensions::CheckSpecVersion,
		signed_extensions::CheckTxVersion,
		signed_extensions::CheckNonce,
		signed_extensions::CheckGenesis<T>,
		signed_extensions::CheckMortality<T>,
		signed_extensions::ChargeAssetTxPayment<T>,
		signed_extensions::ChargeTransactionPayment,
		CheckMetadataHash,
	),
>;

/// The [`CheckMetadataHash`] signed extension.
pub struct CheckMetadataHash {
	// Eventually we might provide or calculate the metadata hash here,
	// but for now we never provide a hash and so this is empty.
}

impl<T: Config> subxt::config::ExtrinsicParams<T> for CheckMetadataHash {
	type OtherParams = ();

	fn new<Client: OfflineClientT<T>>(
		nonce: u64,
		client: Client,
		other_params: Self::OtherParams,
	) -> Result<Self, ExtrinsicParamsError> {
		Ok(CheckMetadataHash {})
	}
}

impl subxt::config::ExtrinsicParamsEncoder for CheckMetadataHash {
	fn encode_extra_to(&self, v: &mut Vec<u8>) {
		// A single 0 byte in the TX payload indicates that the chain should
		// _not_ expect any metadata hash to exist in the signer payload.
		0u8.encode_to(v);
	}
	fn encode_additional_to(&self, v: &mut Vec<u8>) {
		// We provide no metadata hash in the signer payload to align with the above.
		None::<()>.encode_to(v);
	}
}

impl<T: Config> subxt::config::SignedExtension<T> for CheckMetadataHash {
	type Decoded = ();
	fn matches(
		identifier: &str,
		_type_id: u32,
		_types: &sp_runtime::scale_info::PortableRegistry,
	) -> bool {
		identifier == "CheckMetadataHash"
	}
}

/// Is metadata checking enabled or disabled?
// Dev note: The "Disabled" and "Enabled" variant names match those that the
// signed extension will be encoded with, in order that DecodeAsType will work
// properly.
#[derive(
	PartialEq,
	Default,
	Eq,
	Copy,
	Clone,
	Debug,
	scale_encode::EncodeAsType,
	scale_decode::DecodeAsType,
	serde::Serialize,
	serde::Deserialize,
	scale_info::TypeInfo,
)]
pub enum CheckMetadataHashMode {
	/// No hash was provided in the signer payload.
	#[default]
	Disabled,
	/// A hash was provided in the signer payload.
	Enabled,
}

impl CheckMetadataHashMode {
	/// Is metadata checking enabled or disabled for this transaction?
	pub fn is_enabled(&self) -> bool {
		match self {
			CheckMetadataHashMode::Disabled => false,
			CheckMetadataHashMode::Enabled => true,
		}
	}
}

impl Config for SpacewalkRuntime {
	type AssetId = ();
	type Hash = H256;
	type Header = subxt::config::substrate::SubstrateHeader<BlockNumber, Self::Hasher>;
	type Hasher = BlakeTwo256;
	type AccountId = AccountId;
	type Address = Address;
	type Signature = MultiSignature;
	type ExtrinsicParams = CustomExtrinsicParams<Self>;
}
