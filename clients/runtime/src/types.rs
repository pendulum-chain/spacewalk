use subxt::ext::sp_core::ed25519::Pair as KeyPair; // TODO maybe change this back to sr25519

pub use metadata_aliases::*;

use crate::{metadata, Config, SpacewalkRuntime};

// pub use spacewalk_primitives::CurrencyId;

pub type Balance = u128;
pub type BlockNumber = u32;
pub type Index = u32;
pub type H256 = subxt::ext::sp_core::H256;
pub type SpacewalkSigner = subxt::tx::PairSigner<SpacewalkRuntime, KeyPair>;
// pub type FixedU128 = sp_arithmetic::fixed_point::FixedU128;
// pub type FixedU128 = sp_runtime::FixedU128;

pub type StellarPublicKey = [u8; 32];

cfg_if::cfg_if! {
	if #[cfg(feature = "multi-address")] {
		use sp_runtime::{traits::{IdentifyAccount, Verify}, MultiAddress, MultiSignature};

		type Signature = MultiSignature;
		pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
		pub type Address = MultiAddress<AccountId, ()>;
	} else {
		pub type AccountId = subxt::ext::sp_runtime::AccountId32;
		pub type Address = AccountId;
	}
}

mod metadata_aliases {
	pub use metadata::{
		runtime_types::{
			sp_arithmetic::fixed_point::FixedU128, vault_registry::types::VaultStatus,
		},
		vault_registry::events::{
			DepositCollateral as DepositCollateralEvent, LiquidateVault as LiquidateVaultEvent,
			RegisterAddress as RegisterAddressEvent, RegisterVault as RegisterVaultEvent,
		},
	};

	pub use crate::metadata::runtime_types::spacewalk_primitives::CurrencyId;

	use super::*;

	// pub type UnsignedFixedPoint = FixedU128;

	pub type SpacewalkHeader = <SpacewalkRuntime as Config>::Header;

	pub type SpacewalkVault = metadata::runtime_types::vault_registry::types::Vault<
		AccountId,
		BlockNumber,
		Balance,
		CurrencyId,
		FixedU128,
	>;
	pub type VaultId =
		metadata::runtime_types::spacewalk_primitives::VaultId<AccountId, CurrencyId>;
	pub type VaultCurrencyPair =
		metadata::runtime_types::spacewalk_primitives::VaultCurrencyPair<CurrencyId>;
}

mod dispatch_error {
	use crate::metadata::{
		runtime_types::sp_runtime::{ArithmeticError, ModuleError, TokenError, TransactionalError},
		DispatchError,
	};

	type RichTokenError = sp_runtime::TokenError;
	type RichArithmeticError = sp_runtime::ArithmeticError;
	type RichDispatchError = sp_runtime::DispatchError;
	type RichModuleError = sp_runtime::ModuleError;
	type RichTransactionalError = sp_runtime::TransactionalError;

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

	convert_enum!(RichArithmeticError, ArithmeticError, Underflow, Overflow, DivisionByZero,);

	convert_enum!(RichTransactionalError, TransactionalError, LimitReached, NoLayer,);

	impl From<RichDispatchError> for DispatchError {
		fn from(value: RichDispatchError) -> Self {
			match value {
				RichDispatchError::Other(_) => DispatchError::Other,
				RichDispatchError::CannotLookup => DispatchError::CannotLookup,
				RichDispatchError::BadOrigin => DispatchError::BadOrigin,
				RichDispatchError::Module(RichModuleError { index, error, .. }) =>
					DispatchError::Module(ModuleError { index, error }),
				RichDispatchError::ConsumerRemaining => DispatchError::ConsumerRemaining,
				RichDispatchError::NoProviders => DispatchError::NoProviders,
				RichDispatchError::TooManyConsumers => DispatchError::TooManyConsumers,
				RichDispatchError::Token(token_error) => DispatchError::Token(token_error.into()),
				RichDispatchError::Arithmetic(arithmetic_error) =>
					DispatchError::Arithmetic(arithmetic_error.into()),
				RichDispatchError::Transactional(transactional_error) =>
					DispatchError::Transactional(transactional_error.into()),
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
