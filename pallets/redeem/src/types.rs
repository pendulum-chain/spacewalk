use currency::Amount;
use frame_system::pallet_prelude::BlockNumberFor;
pub use primitives::redeem::{RedeemRequest, RedeemRequestStatus};
use primitives::VaultId;
pub use vault_registry::types::CurrencyId;

use crate::Config;

pub(crate) type BalanceOf<T> = <T as vault_registry::Config>::Balance;

pub(crate) type DefaultVaultId<T> = VaultId<<T as frame_system::Config>::AccountId, CurrencyId<T>>;

pub type DefaultRedeemRequest<T> = RedeemRequest<
	<T as frame_system::Config>::AccountId,
	BlockNumberFor<T>,
	BalanceOf<T>,
	CurrencyId<T>,
>;

pub trait RedeemRequestExt<T: Config> {
	fn amount(&self) -> Amount<T>;
	fn fee(&self) -> Amount<T>;
	fn premium(&self) -> Amount<T>;
	fn transfer_fee(&self) -> Amount<T>;
}

impl<T: Config> RedeemRequestExt<T>
	for RedeemRequest<T::AccountId, BlockNumberFor<T>, BalanceOf<T>, CurrencyId<T>>
{
	fn amount(&self) -> Amount<T> {
		Amount::new(self.amount, self.asset)
	}
	fn fee(&self) -> Amount<T> {
		Amount::new(self.fee, self.asset)
	}
	fn premium(&self) -> Amount<T> {
		Amount::new(self.premium, self.vault.collateral_currency())
	}
	fn transfer_fee(&self) -> Amount<T> {
		Amount::new(self.transfer_fee, self.asset)
	}
}
