use currency::Amount;
use frame_support::traits::Get;
pub use primitives::issue::{IssueRequest, IssueRequestStatus};
use primitives::VaultId;
pub use vault_registry::types::CurrencyId;

use crate::Config;

pub(crate) type BalanceOf<T> = <T as vault_registry::Config>::Balance;
pub(crate) type DefaultVaultId<T> = VaultId<<T as frame_system::Config>::AccountId, CurrencyId<T>>;

pub type DefaultIssueRequest<T> = IssueRequest<
	<T as frame_system::Config>::AccountId,
	<T as frame_system::Config>::BlockNumber,
	BalanceOf<T>,
	CurrencyId<T>,
>;

pub trait IssueRequestExt<T: Config> {
	fn amount(&self) -> Amount<T>;
	fn fee(&self) -> Amount<T>;
	fn griefing_collateral(&self) -> Amount<T>;
}

impl<T: Config> IssueRequestExt<T> for DefaultIssueRequest<T> {
	fn amount(&self) -> Amount<T> {
		Amount::new(self.amount, self.asset)
	}
	fn fee(&self) -> Amount<T> {
		Amount::new(self.fee, self.asset)
	}
	fn griefing_collateral(&self) -> Amount<T> {
		Amount::new(self.griefing_collateral, T::GetGriefingCollateralCurrencyId::get())
	}
}
