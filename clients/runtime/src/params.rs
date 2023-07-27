use codec::Encode;
use sp_runtime::app_crypto::sp_core;
use subxt::tx::ExtrinsicParams;
use primitives::CurrencyId;
use runtime::H256;

#[derive(Encode, Debug, Clone, Eq, PartialEq)]
pub struct ChargeAssetTxPayment {
    pub tip: u128,
    pub asset_id:Option<CurrencyId>
}

#[derive(Encode, Debug, Clone, Eq, PartialEq)]
pub struct SpacewalkExtraParams {
    pub era: sp_runtime::generic::Era,
    pub nonce: u64,
    pub charge: ChargeAssetTxPayment
}

#[derive(Encode, Debug, Clone, Eq, PartialEq)]
pub struct SpacewalkAdditionalParams {
    pub spec_version: u32,
    pub tx_version: u32,
    pub genesis_hash: sp_core::H256,
    pub mortality_hash: sp_core::H256,
}

#[derive(Encode, Debug, Clone, Eq, PartialEq)]
pub struct SpacewalkExtrinsicParams {
    pub extra_params: SpacewalkExtraParams,
    pub additional_params: SpacewalkAdditionalParams
}

impl ExtrinsicParams<u64,sp_core::H256> for SpacewalkExtrinsicParams {
    type OtherParams = (
        sp_core::H256,
        sp_runtime::generic::Era,
        ChargeAssetTxPayment
    );

    fn new(spec_version: u32, tx_version: u32, nonce: u64, genesis_hash: primitives::H256, other_params: Self::OtherParams) -> Self {

        let (mortality_hash, era, charge) = other_params;

        let extra_params = SpacewalkExtraParams {
            era,
            nonce,
            charge,
        };

        let additional_params = SpacewalkAdditionalParams {
            spec_version,
            tx_version,
            genesis_hash,
            mortality_hash,
        };

        Self {
            extra_params,
            additional_params,
        }
    }

    fn encode_extra_to(&self, v: &mut Vec<u8>) {
        self.extra_params.encode_to(v)
    }

    fn encode_additional_to(&self, v: &mut Vec<u8>) {
        self.additional_params.encode_to(v)
    }
}