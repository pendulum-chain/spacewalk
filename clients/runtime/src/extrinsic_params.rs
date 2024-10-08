use codec::Encode;
use subxt::{
	client::OfflineClientT,
	config::{
		ExtrinsicParamsEncoder, ExtrinsicParamsError, PolkadotExtrinsicParams, SignedExtension,
	},
	Config,
};

#[cfg(not(feature = "standalone-metadata"))]
use subxt::config::signed_extensions;

// Check features to decide which extrinsic params to use
cfg_if::cfg_if! {
	 if #[cfg(feature = "standalone-metadata")] {
		pub type UsedExtrinsicParams<T> = PolkadotExtrinsicParams<T>;
	} else {
		pub type UsedExtrinsicParams<T> = CustomExtrinsicParams<T>;
	}
}

#[cfg(not(feature = "standalone-metadata"))]
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

/// The following implementation is adopted from [here](https://github.com/paritytech/subxt/pull/1590).
/// The CheckMetadataHash is added to subxt in v0.37.0, see [here](https://github.com/paritytech/subxt/releases/tag/v0.37.0)
/// so we can remove the custom implementation once we upgrade to that version.

/// The [`CheckMetadataHash`] signed extension.
pub struct CheckMetadataHash {
	// Eventually we might provide or calculate the metadata hash here,
	// but for now we never provide a hash and so this is empty.
}

impl<T: Config> subxt::config::ExtrinsicParams<T> for CheckMetadataHash {
	type OtherParams = ();

	fn new<Client: OfflineClientT<T>>(
		_nonce: u64,
		_client: Client,
		_other_params: Self::OtherParams,
	) -> Result<Self, ExtrinsicParamsError> {
		Ok(CheckMetadataHash {})
	}
}

impl ExtrinsicParamsEncoder for CheckMetadataHash {
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

impl<T: Config> SignedExtension<T> for CheckMetadataHash {
	type Decoded = ();
	fn matches(
		identifier: &str,
		_type_id: u32,
		_types: &sp_runtime::scale_info::PortableRegistry,
	) -> bool {
		identifier == "CheckMetadataHash"
	}
}
