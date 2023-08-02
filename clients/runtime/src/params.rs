use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use codec::{Encode, Decode, Compact};
use sp_runtime::app_crypto::sp_core;
use sp_runtime::generic::Era;

use subxt::{Config, tx::ExtrinsicParams};
use subxt::ext::sp_core::H256;
use subxt::utils::Encoded;
use primitives::{CurrencyId, };

#[derive(Encode, Decode, Debug, Clone, Eq, PartialEq)]
pub struct ChargeAssetTxPayment {
    #[codec(compact)]
    pub tip: u128,
    pub asset_id:Option<CurrencyId>
}

impl  Default for ChargeAssetTxPayment {
    fn default() -> Self {
        Self {
            tip: u128::default(),
            asset_id: None,
        }
    }
}

#[derive(Encode, Decode, Debug, Clone, Eq, PartialEq)]
pub struct SpacewalkOtherParams<Hash> {
    pub era: Era,
    pub mortality_checkpoint: Option<Hash>,
    pub charge: ChargeAssetTxPayment
}

impl <Hash> Default for SpacewalkOtherParams<Hash> {
    fn default() -> Self {
        Self {
            era: Era::Immortal,
            mortality_checkpoint: None,
            charge: ChargeAssetTxPayment::default(),
        }
    }
}

#[derive(Encode, Decode, Debug, Clone, Eq, PartialEq)]
pub struct SpacewalkExtrinsicParams<T: Config> {
    pub era: Era,
    pub nonce: T::Index,
    pub spec_version: u32,
    pub transaction_version: u32,
    pub genesis_hash: T::Hash,
    pub mortality_checkpoint: T::Hash,
    pub charge_asset_tx_payment: ChargeAssetTxPayment,
    marker: std::marker::PhantomData<T>,
}

// impl<T: Config, Tip: Debug + Encode + 'static> Debug for SpacewalkExtrinsicParams<T, Tip, CurrencyId> {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         todo!()
//     }
// }

impl <T:Config + Debug> ExtrinsicParams<T::Index, T::Hash> for SpacewalkExtrinsicParams<T> {
    type OtherParams = SpacewalkOtherParams<T::Hash>;

    fn new(spec_version: u32, tx_version: u32, nonce: T::Index, genesis_hash: T::Hash, other_params: Self::OtherParams) -> Self {
        Self {
            era: other_params.era,
            nonce,
            spec_version,
            transaction_version: tx_version,
            genesis_hash: genesis_hash.clone(),
            mortality_checkpoint: other_params.mortality_checkpoint.unwrap_or(genesis_hash),
            charge_asset_tx_payment: other_params.charge,
            marker: std::marker::PhantomData,
        }
    }

    fn encode_extra_to(&self, v: &mut Vec<u8>) {
        let nonce: u64 = self.nonce.into() ;
        let extra = Encoded(self.charge_asset_tx_payment.encode());
        (self.era, Compact(nonce), extra).encode_to(v);
    }

    fn encode_additional_to(&self, v: &mut Vec<u8>) {
        (
            self.spec_version.clone(),
            self.transaction_version.clone(),
            self.genesis_hash,
            self.mortality_checkpoint
        ).encode_to(v)
    }
}
