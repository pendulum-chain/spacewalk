use crate::Config;
use currency::Amount;
use frame_support::traits::Get;
pub use primitives::replace::{ReplaceRequest, ReplaceRequestStatus};
use primitives::VaultId;
use sp_runtime::DispatchError;
pub use vault_registry::types::CurrencyId;

pub(crate) type BalanceOf<T> = <T as vault_registry::Config>::Balance;

pub(crate) type DefaultVaultId<T> = VaultId<<T as frame_system::Config>::AccountId, CurrencyId<T>>;

pub type DefaultReplaceRequest<T> = ReplaceRequest<
	<T as frame_system::Config>::AccountId,
	<T as frame_system::Config>::BlockNumber,
	BalanceOf<T>,
	CurrencyId<T>,
>;

pub trait ReplaceRequestExt<T: Config> {
	fn amount(&self) -> Amount<T>;
	fn griefing_collateral(&self) -> Amount<T>;
	fn collateral(&self) -> Result<Amount<T>, DispatchError>;
}

impl<T: Config> ReplaceRequestExt<T> for DefaultReplaceRequest<T> {
	fn amount(&self) -> Amount<T> {
		Amount::new(self.amount, self.old_vault.wrapped_currency())
	}
	fn griefing_collateral(&self) -> Amount<T> {
		Amount::new(self.griefing_collateral, T::GetGriefingCollateralCurrencyId::get())
	}
	fn collateral(&self) -> Result<Amount<T>, DispatchError> {
		Ok(Amount::new(self.collateral, self.new_vault.collateral_currency()))
	}
}
