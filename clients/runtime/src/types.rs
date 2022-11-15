// TODO maybe change this back to sr25519
use subxt::ext::sp_core::ed25519::Pair as KeyPair;

pub use metadata_aliases::*;
pub use primitives::CurrencyId;

use crate::{metadata, Config, SpacewalkRuntime, SS58_PREFIX};

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

	use super::*;

	// pub use crate::metadata::runtime_types::spacewalk_primitives::CurrencyId;

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

pub trait PrettyPrint {
	fn pretty_print(&self) -> String;
}

mod account_id {
	use subxt::ext::sp_core::crypto::Ss58Codec;

	use super::*;

	impl PrettyPrint for AccountId {
		fn pretty_print(&self) -> String {
			self.to_ss58check_with_version(SS58_PREFIX.into())
		}
	}
}

mod vault_id {
	use super::*;

	type RichVaultId = primitives::VaultId<AccountId, primitives::CurrencyId>;

	impl crate::VaultId {
		pub fn new(
			account_id: AccountId,
			collateral_currency: CurrencyId,
			wrapped_currency: CurrencyId,
		) -> Self {
			Self {
				account_id,
				currencies: VaultCurrencyPair {
					collateral: collateral_currency,
					wrapped: wrapped_currency,
				},
			}
		}

		pub fn collateral_currency(&self) -> CurrencyId {
			self.currencies.collateral.clone()
		}

		pub fn wrapped_currency(&self) -> CurrencyId {
			self.currencies.wrapped.clone()
		}
	}

	impl PrettyPrint for crate::VaultId {
		fn pretty_print(&self) -> String {
			let collateral_currency: CurrencyId = self.collateral_currency();
			let wrapped_currency: CurrencyId = self.wrapped_currency();
			format!(
				"{}[{:?}->{:?}]",
				self.account_id.pretty_print(),
				collateral_currency,
				wrapped_currency,
			)
		}
	}

	impl From<crate::VaultId> for RichVaultId {
		fn from(value: crate::VaultId) -> Self {
			Self {
				account_id: value.account_id,
				currencies: primitives::VaultCurrencyPair {
					collateral: value.currencies.collateral,
					wrapped: value.currencies.wrapped,
				},
			}
		}
	}

	impl From<RichVaultId> for crate::VaultId {
		fn from(value: RichVaultId) -> Self {
			Self {
				account_id: value.account_id,
				currencies: crate::VaultCurrencyPair {
					collateral: value.currencies.collateral,
					wrapped: value.currencies.wrapped,
				},
			}
		}
	}

	impl serde::Serialize for crate::VaultId {
		fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: serde::Serializer,
		{
			let value: RichVaultId = self.clone().into();
			value.serialize(serializer)
		}
	}

	impl<'de> serde::Deserialize<'de> for crate::VaultId {
		fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
		where
			D: serde::Deserializer<'de>,
		{
			let value = RichVaultId::deserialize(deserializer)?;
			Ok(value.into())
		}
	}

	impl std::hash::Hash for crate::VaultId {
		fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
			let vault: RichVaultId = self.clone().into();
			vault.hash(state)
		}
	}
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
