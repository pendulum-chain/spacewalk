pub use subxt::ext::sp_core::sr25519::Pair as KeyPair;

pub use metadata_aliases::*;
pub use primitives::{CurrencyId, CurrencyId::Token, TokenSymbol};

use crate::{metadata, Config, SpacewalkRuntime, SS58_PREFIX};

pub type AccountId = subxt::ext::sp_runtime::AccountId32;
pub type Address = subxt::ext::sp_runtime::MultiAddress<AccountId, u32>;
pub type Balance = u128;
pub type BlockNumber = u32;
pub type Index = u32;
pub type H256 = subxt::ext::sp_core::H256;
pub type SpacewalkSigner = subxt::tx::PairSigner<SpacewalkRuntime, KeyPair>;
pub type FixedU128 = sp_arithmetic::FixedU128;

pub type IssueId = H256;

pub type StellarPublicKey = [u8; 32];

mod metadata_aliases {
	use std::collections::HashMap;

	pub use metadata::{
		issue::events::{
			CancelIssue as CancelIssueEvent, ExecuteIssue as ExecuteIssueEvent,
			RequestIssue as RequestIssueEvent,
		},
		oracle::events::FeedValues as FeedValuesEvent,
		replace::events::{
			AcceptReplace as AcceptReplaceEvent, CancelReplace as CancelReplaceEvent,
			ExecuteReplace as ExecuteReplaceEvent, RequestReplace as RequestReplaceEvent,
			WithdrawReplace as WithdrawReplaceEvent,
		},
		runtime_types::{
			frame_system::pallet::Error as SystemPalletError,
			issue::pallet::Error as IssuePalletError,
			redeem::pallet::Error as RedeemPalletError,
			security::{
				pallet::{Call as SecurityCall, Error as SecurityPalletError},
				types::{ErrorCode, StatusCode},
			},
			spacewalk_primitives::{
				issue::IssueRequestStatus, oracle::Key as OracleKey, redeem::RedeemRequestStatus,
				replace::ReplaceRequestStatus,
			},
			vault_registry::{pallet::Error as VaultRegistryPalletError, types::VaultStatus},
		},
		security::events::UpdateActiveBlock as UpdateActiveBlockEvent,
		tokens::events::Endowed as EndowedEvent,
		vault_registry::events::{
			DepositCollateral as DepositCollateralEvent, LiquidateVault as LiquidateVaultEvent,
			RegisterAddress as RegisterAddressEvent, RegisterVault as RegisterVaultEvent,
		},
	};

	use super::*;

	pub type SpacewalkReplaceRequest =
		metadata::runtime_types::spacewalk_primitives::replace::ReplaceRequest<
			AccountId,
			BlockNumber,
			Balance,
			CurrencyId,
		>;

	// pub use crate::metadata::runtime_types::spacewalk_primitives::CurrencyId;

	pub type SpacewalkIssueRequest =
		metadata::runtime_types::spacewalk_primitives::issue::IssueRequest<
			AccountId,
			BlockNumber,
			Balance,
			CurrencyId,
		>;

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

	pub type IssueRequestsMap = HashMap<IssueId, SpacewalkIssueRequest>;

	#[cfg(feature = "standalone-metadata")]
	pub type EncodedCall = metadata::runtime_types::spacewalk_runtime_standalone::RuntimeCall;
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
				RichDispatchError::Exhausted |
				sp_runtime::DispatchError::Corruption |
				sp_runtime::DispatchError::Unavailable => todo!(),
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
