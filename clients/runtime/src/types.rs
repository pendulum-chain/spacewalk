use crate::{metadata, Config, InterBtcRuntime};
pub use metadata_aliases::*;
use subxt::sp_core::{crypto::Ss58Codec, sr25519::Pair as KeyPair};

pub use primitives::{
    CurrencyId,
    CurrencyId::Token,
    TokenSymbol::{DOT, INTERBTC, INTR, KBTC, KINT, KSM},
};

pub use currency_id::CurrencyIdExt;
pub use module_btc_relay::{RichBlockHeader, MAIN_CHAIN_ID};

pub type AccountId = subxt::sp_runtime::AccountId32;
pub type Balance = primitives::Balance;
pub type Index = u32;
pub type BlockNumber = u32;
pub type H160 = subxt::sp_core::H160;
pub type H256 = subxt::sp_core::H256;
pub type U256 = subxt::sp_core::U256;

pub type InterBtcSigner = subxt::PairSigner<InterBtcRuntime, subxt::DefaultExtra<InterBtcRuntime>, KeyPair>;

pub type BtcAddress = module_btc_relay::BtcAddress;

pub type FixedU128 = sp_arithmetic::FixedU128;

mod metadata_aliases {
    use super::*;

    pub type RedeemEvent = metadata::spacewalk::events::Redeem;

    pub type InterBtcHeader = <InterBtcRuntime as Config>::Header;
}

mod currency_id {
    use super::*;

    pub trait CurrencyIdExt {
        fn inner(&self) -> primitives::TokenSymbol;
    }

    impl CurrencyIdExt for CurrencyId {
        fn inner(&self) -> primitives::TokenSymbol {
            match self {
                Token(x) => *x,
            }
        }
    }
}

mod dispatch_error {
    use crate::metadata::{
        runtime_types::sp_runtime::{ArithmeticError, ModuleError, TokenError},
        DispatchError,
    };

    type RichTokenError = sp_runtime::TokenError;
    type RichArithmeticError = sp_runtime::ArithmeticError;
    type RichDispatchError = sp_runtime::DispatchError;
    type RichModuleError = sp_runtime::ModuleError;

    macro_rules! convert_enum{($src: ident, $dst: ident, $($variant: ident,)*)=> {
        impl From<$src> for $dst {
            fn from(src: $src) -> Self {
                match src {
                    $($src::$variant => Self::$variant,)*
                }
            }
        }
    }}

    convert_enum!(
        RichTokenError,
        TokenError,
        NoFunds,
        WouldDie,
        BelowMinimum,
        CannotCreate,
        UnknownAsset,
        Frozen,
        Unsupported,
    );

    convert_enum!(
        RichArithmeticError,
        ArithmeticError,
        Underflow,
        Overflow,
        DivisionByZero,
    );

    impl From<RichDispatchError> for DispatchError {
        fn from(value: RichDispatchError) -> Self {
            match value {
                RichDispatchError::Other(_) => DispatchError::Other,
                RichDispatchError::CannotLookup => DispatchError::CannotLookup,
                RichDispatchError::BadOrigin => DispatchError::BadOrigin,
                RichDispatchError::Module(RichModuleError { index, error, .. }) => {
                    DispatchError::Module(ModuleError { index, error })
                }
                RichDispatchError::ConsumerRemaining => DispatchError::ConsumerRemaining,
                RichDispatchError::NoProviders => DispatchError::NoProviders,
                RichDispatchError::TooManyConsumers => DispatchError::TooManyConsumers,
                RichDispatchError::Token(token_error) => DispatchError::Token(token_error.into()),
                RichDispatchError::Arithmetic(arithmetic_error) => DispatchError::Arithmetic(arithmetic_error.into()),
            }
        }
    }

    impl<'de> serde::Deserialize<'de> for DispatchError {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let value = RichDispatchError::deserialize(deserializer)?;
            Ok(value.into())
        }
    }
}
