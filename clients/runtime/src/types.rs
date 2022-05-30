use crate::{metadata, Config, SpacewalkRuntime};
pub use metadata_aliases::*;
use subxt::sp_core::sr25519::Pair as KeyPair;

pub type AccountId = spacewalk_runtime::AccountId;
pub type Balance = spacewalk_runtime::Balance;
pub type Index = u32;
pub type BlockNumber = u32;
pub type H256 = subxt::sp_core::H256;

pub type SpacewalkSigner = subxt::PairSigner<SpacewalkRuntime, KeyPair>;

pub type FixedU128 = sp_arithmetic::FixedU128;

mod metadata_aliases {
    use super::*;

    pub type DepositEvent = metadata::spacewalk::events::Deposit;
    pub type RedeemEvent = metadata::spacewalk::events::Redeem;

    pub type SpacewalkHeader = <SpacewalkRuntime as Config>::Header;
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
