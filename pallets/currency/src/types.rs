use frame_support::dispatch::DispatchError;
use orml_traits::MultiCurrency;

use crate::Config;

pub type CurrencyId<T> = <<T as orml_currencies::Config>::MultiCurrency as MultiCurrency<
	<T as frame_system::Config>::AccountId,
>>::CurrencyId;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

pub(crate) type BalanceOf<T> = <<T as orml_currencies::Config>::MultiCurrency as MultiCurrency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

pub(crate) type SignedFixedPoint<T> = <T as Config>::SignedFixedPoint;

pub(crate) type UnsignedFixedPoint<T> = <T as Config>::UnsignedFixedPoint;

pub(crate) type SignedInner<T> = <T as Config>::SignedInner;
pub trait CurrencyConversion<Amount, CurrencyId> {
	fn convert(amount: &Amount, to: CurrencyId) -> Result<Amount, DispatchError>;
}
