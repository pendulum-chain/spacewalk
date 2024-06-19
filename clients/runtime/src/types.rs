pub use subxt::ext::sp_core::sr25519::Pair as KeyPair;
use subxt::utils::Static;
pub use metadata_aliases::*;
pub use primitives::{CurrencyId, TextMemo};
use std::str::from_utf8;
use sp_runtime::{traits::BlakeTwo256, generic};

use crate::{metadata, Config, SpacewalkRuntime};

pub type AccountId = subxt::utils::AccountId32;
pub type Address = subxt::ext::sp_runtime::MultiAddress<AccountId, u32>;
pub type Balance = u128;
pub type BlockNumber = u32;
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
pub type Nonce = u32;
pub type H256 = subxt::ext::sp_core::H256;
pub type SpacewalkSigner = subxt::tx::PairSigner<SpacewalkRuntime, KeyPair>;
pub type FixedU128 = sp_arithmetic::FixedU128;

pub use substrate_stellar_sdk as stellar;

pub type IssueId = H256;

pub type StellarPublicKeyRaw = [u8; 32];

mod metadata_aliases {
	use std::collections::HashMap;

	pub use metadata::{
		issue::events::{
			CancelIssue as CancelIssueEvent, ExecuteIssue as ExecuteIssueEvent,
			RequestIssue as RequestIssueEvent,
		},
		oracle::events::AggregateUpdated as AggregateUpdatedEvent,
		redeem::events::{
			ExecuteRedeem as ExecuteRedeemEvent, RequestRedeem as RequestRedeemEvent,
		},
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
			Static<CurrencyId>,
		>;

	pub type SpacewalkRedeemRequest =
		metadata::runtime_types::spacewalk_primitives::redeem::RedeemRequest<
			AccountId,
			BlockNumber,
			Balance,
			Static<CurrencyId>,
		>;

	pub type SpacewalkIssueRequest =
		metadata::runtime_types::spacewalk_primitives::issue::IssueRequest<
			AccountId,
			BlockNumber,
			Balance,
			Static<CurrencyId>,
		>;

	pub type SpacewalkHeader = <SpacewalkRuntime as Config>::Header;

	pub type SpacewalkVault = metadata::runtime_types::vault_registry::types::Vault<
		AccountId,
		BlockNumber,
		Balance,
		Static<CurrencyId>,
		Static<FixedU128>,
	>;
	pub type VaultId =
		metadata::runtime_types::spacewalk_primitives::VaultId<AccountId, Static<CurrencyId>>;

	pub type VaultCurrencyPair =
		metadata::runtime_types::spacewalk_primitives::VaultCurrencyPair<Static<CurrencyId>>;

	pub type IssueRequestsMap = HashMap<IssueId, SpacewalkIssueRequest>;
	pub type IssueIdLookup = HashMap<TextMemo, IssueId>;

	cfg_if::cfg_if! {
		if #[cfg(feature = "standalone-metadata")] {
			pub type EncodedCall = metadata::runtime_types::spacewalk_runtime_standalone_testnet::RuntimeCall;
		} else if #[cfg(feature = "parachain-metadata-pendulum")] {
			pub type EncodedCall = metadata::runtime_types::pendulum_runtime::RuntimeCall;
		} else if #[cfg(feature = "parachain-metadata-amplitude")] {
			pub type EncodedCall = metadata::runtime_types::amplitude_runtime::RuntimeCall;
		} else if #[cfg(feature = "parachain-metadata-foucoco")] {
			pub type EncodedCall = metadata::runtime_types::foucoco_runtime::RuntimeCall;
		}
	}
}

pub mod currency_id {
	use primitives::Asset;

	use super::*;
	use crate::Error;

	pub trait CurrencyIdExt {
		fn inner(&self) -> Result<String, Error>;
	}

	impl CurrencyIdExt for CurrencyId {
		fn inner(&self) -> Result<String, Error> {
			match self {
				CurrencyId::Native => Ok("Native".to_owned()),
				CurrencyId::XCM(foreign_currency_id) => Ok(format!("XCM({})", foreign_currency_id)),
				CurrencyId::Stellar(stellar_asset) => match stellar_asset {
					Asset::StellarNative => Ok("XLM".to_owned()),
					Asset::AlphaNum4 { code, issuer } => Ok(format!(
						"Stellar({:?}:{:?})",
						from_utf8(code).unwrap_or_default(),
						from_utf8(
							stellar::PublicKey::from_binary(*issuer).to_encoding().as_slice()
						)
						.unwrap_or_default()
					)
					.replace('\"', "")),
					Asset::AlphaNum12 { code, issuer } => Ok(format!(
						"Stellar({:?}:{:?})",
						from_utf8(code).unwrap_or_default(),
						from_utf8(
							stellar::PublicKey::from_binary(*issuer).to_encoding().as_slice()
						)
						.unwrap_or_default()
					)
					.replace('\"', "")),
				},
				CurrencyId::ZenlinkLPToken(token1_id, token1_type, token2_id, token2_type) =>
					Ok(format!(
						"ZenlinkLPToken({},{},{},{})",
						token1_id, token1_type, token2_id, token2_type
					)),
				CurrencyId::Token(token_id) => Ok(format!("Token({})", token_id)),
			}
		}
	}
}

pub trait PrettyPrint {
	fn pretty_print(&self) -> String;
}

mod account_id {
	//use sp_core::crypto::Ss58Codec;

	use super::*;

	impl PrettyPrint for AccountId {
		fn pretty_print(&self) -> String {
			"self.into()".to_string()
		}
	}
}

mod vault_id {
	use super::*;

	type RichVaultId = primitives::VaultId<AccountId, primitives::CurrencyId>;

	type RichVaultHashable = primitives::VaultId<sp_runtime::AccountId32, primitives::CurrencyId>;
	//type RichVaultId = VaultId;

	impl crate::VaultId {
		pub fn new(
			account_id: AccountId,
			collateral_currency: CurrencyId,
			wrapped_currency: CurrencyId,
		) -> Self {
			Self {
				account_id,
				currencies: VaultCurrencyPair {
					collateral: Static(collateral_currency),
					wrapped: Static(wrapped_currency),
				},
			}
		}

		pub fn collateral_currency(&self) -> CurrencyId {
			*self.currencies.collateral
		}

		pub fn wrapped_currency(&self) -> CurrencyId {
			*self.currencies.wrapped
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
					collateral: *value.currencies.collateral,
					wrapped: *value.currencies.wrapped,
				},
			}
		}
	}

	impl From<RichVaultId> for crate::VaultId {
		fn from(value: RichVaultId) -> Self {
			Self {
				account_id: value.account_id,
				currencies: crate::VaultCurrencyPair {
					collateral: Static(value.currencies.collateral),
					wrapped: Static(value.currencies.wrapped),
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
			// Extract rich vault, then create a hashable version of it 
			// defined with sp_runtime::AccountId32, which is hashable
			let vault: RichVaultId = self.clone().into();
			let vault_hashable = RichVaultHashable {
				account_id: sp_runtime::AccountId32::new(vault.account_id.0),
					currencies: primitives::VaultCurrencyPair {
						collateral: vault.currencies.collateral,
						wrapped:  vault.currencies.collateral,
					},
			};
			vault_hashable.hash(state)
		}
	}
}

mod dispatch_error {
	use crate::metadata::{
		runtime_types::{
			sp_arithmetic::ArithmeticError,
			sp_runtime::{ModuleError, TokenError, TransactionalError},
		},
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
		BelowMinimum,
		CannotCreate,
		UnknownAsset,
		Frozen,
		Unsupported,
		FundsUnavailable,
		OnlyProvider,
		CannotCreateHold,
		NotExpendable,
		Blocked,
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
				RichDispatchError::Exhausted => DispatchError::Exhausted,
				sp_runtime::DispatchError::Corruption => DispatchError::Corruption,
				sp_runtime::DispatchError::Unavailable => DispatchError::Unavailable,
				sp_runtime::DispatchError::RootNotAllowed => todo!(),
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
