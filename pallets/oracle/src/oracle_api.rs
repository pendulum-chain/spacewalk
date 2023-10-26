use crate::{types::BalanceOf, Config, CurrencyId, DispatchError, Pallet};
pub trait OracleApi<Balance, CurrencyId> {
	fn currency_to_usd(
		amount: &Balance,
		currency_id: &CurrencyId,
	) -> Result<Balance, DispatchError>;
}

impl<T: Config> OracleApi<BalanceOf<T>, CurrencyId> for Pallet<T> {
	fn currency_to_usd(
		amount: &BalanceOf<T>,
		currency_id: &CurrencyId,
	) -> Result<BalanceOf<T>, DispatchError> {
		Pallet::<T>::currency_to_usd(amount.clone(), currency_id.clone())
	}
}
