use crate::{metadata, Config, SpacewalkRuntime};
pub use metadata_aliases::*;
use subxt::sp_core::ed25519::Pair as KeyPair;

pub type Balance = u128;
pub type BlockNumber = u32;
pub type Index = u32;
pub type H256 = subxt::sp_core::H256;
pub type SpacewalkSigner = subxt::PairSigner<SpacewalkRuntime, KeyPair>;
pub type FixedU128 = sp_arithmetic::FixedU128;

cfg_if::cfg_if! {
	if #[cfg(feature = "multi-address")] {
		use sp_runtime::{traits::{IdentifyAccount, Verify}, MultiAddress, MultiSignature};

		type Signature = MultiSignature;
		pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
		pub type Address = MultiAddress<AccountId, ()>;
	} else {
		pub type AccountId = subxt::sp_runtime::AccountId32;
		pub type Address = AccountId;
	}
}

mod metadata_aliases {
	use super::*;

	use crate::metadata::runtime_types::spacewalk_primitives::{issue::IssueRequest, VaultId};

	pub use crate::metadata::runtime_types::{
		issue::pallet::Error as IssuePalletError, spacewalk_primitives::CurrencyId,
	};

	// pub type DepositEvent = metadata::spacewalk::events::Deposit;
	// pub type RedeemEvent = metadata::spacewalk::events::Redeem;

	// #[cfg(feature = "standalone-metadata")]
	pub type RequestIssueEvent = metadata::issue::events::RequestIssue;
	// #[cfg(feature = "standalone-metadata")]
	pub type CancelIssueEvent = metadata::issue::events::CancelIssue;
	// #[cfg(feature = "standalone-metadata")]
	pub type ExecuteIssueEvent = metadata::issue::events::ExecuteIssue;

	pub type SpacewalkHeader = <SpacewalkRuntime as Config>::Header;

	pub type DefaultIssueRequest =
		IssueRequest<<SpacewalkRuntime as Config>::AccountId, BlockNumber, Balance, CurrencyId>;

	pub type DefaultVaultId = VaultId<<SpacewalkRuntime as Config>::AccountId, CurrencyId>;
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
