// We allow these lints because they are only caused by the 'mockable' attribute.
#![allow(clippy::forget_non_drop, clippy::swap_ptr_to_ref, clippy::forget_ref, clippy::forget_copy)]
use frame_support::{
	dispatch::{DispatchError, DispatchResult}, log,
	ensure,
};
use orml_traits::{MultiCurrency, MultiReservableCurrency};
use sp_runtime::{
	traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, UniqueSaturatedInto, Zero},
	ArithmeticError, FixedPointNumber,
};
use sp_std::{convert::TryInto, fmt::Debug};

use primitives::{AmountCompatibility, TruncateFixedPointToInt};

use crate::{
	pallet::{self, Config, Error},
	types::*,
};

#[cfg_attr(feature = "testing-utils", derive(Copy))]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Amount<T: Config> {
	amount: BalanceOf<T>,
	currency_id: CurrencyId<T>,
}

#[cfg_attr(feature = "testing-utils", mocktopus::macros::mockable)]
impl<T: Config> Amount<T> {
	pub const fn new(amount: BalanceOf<T>, currency_id: CurrencyId<T>) -> Self {
		Self { amount, currency_id }
	}

	pub fn amount(&self) -> BalanceOf<T> {
		self.amount
	}

	pub fn currency(&self) -> CurrencyId<T> {
		self.currency_id
	}
}

#[cfg_attr(feature = "testing-utils", mocktopus::macros::mockable)]
mod conversions {
	use super::*;

	impl<T: Config> Amount<T> {
		pub fn from_signed_fixed_point(
			amount: SignedFixedPoint<T>,
			currency_id: CurrencyId<T>,
		) -> Result<Self, DispatchError> {
			let amount = amount
				.truncate_to_inner()
				.ok_or(Error::<T>::TryIntoIntError)?
				.try_into()
				.map_err(|_| Error::<T>::TryIntoIntError)?;
			Ok(Self::new(amount, currency_id))
		}

		pub fn to_signed_fixed_point(&self) -> Result<SignedFixedPoint<T>, DispatchError> {
			let signed_inner = TryInto::<SignedInner<T>>::try_into(self.amount)
				.map_err(|_| Error::<T>::TryIntoIntError)?;
			let signed_fixed_point =
				<T as pallet::Config>::SignedFixedPoint::checked_from_integer(signed_inner)
					.ok_or(Error::<T>::TryIntoIntError)?;
			Ok(signed_fixed_point)
		}

		pub fn convert_to(&self, currency_id: CurrencyId<T>) -> Result<Self, DispatchError> {
			T::CurrencyConversion::convert(self, currency_id)
		}
	}
}

#[cfg_attr(feature = "testing-utils", mocktopus::macros::mockable)]
mod math {
	use super::*;

	impl<T: Config> Amount<T> {
		pub fn zero(currency_id: CurrencyId<T>) -> Self {
			Self::new(0u32.into(), currency_id)
		}

		pub fn is_zero(&self) -> bool {
			self.amount.is_zero()
		}

		pub fn ensure_is_compatible_with_target_chain(&self) -> Result<(), DispatchError> {
			if !T::AmountCompatibility::is_compatible_with_target(self.amount) {
				return Err(Error::<T>::IncompatibleAmount.into())
			}
			Ok(())
		}

		pub fn round_to_target_chain(&self) -> Result<Self, DispatchError> {
			let rounded_amount =
				T::AmountCompatibility::round_to_compatible_with_target(self.amount)
					.map_err(|_| Error::<T>::CompatibleRoundingFailed)?;

			Ok(Self::new(rounded_amount, self.currency_id))
		}

		fn checked_fn<F>(
			&self,
			other: &Self,
			f: F,
			err: ArithmeticError,
		) -> Result<Self, DispatchError>
		where
			F: Fn(&BalanceOf<T>, &BalanceOf<T>) -> Option<BalanceOf<T>>,
		{
			if self.currency_id != other.currency_id {
				return Err(Error::<T>::InvalidCurrency.into())
			}
			let amount = f(&self.amount, &other.amount).ok_or(err)?;

			Ok(Self { amount, currency_id: self.currency_id })
		}

		pub fn checked_add(&self, other: &Self) -> Result<Self, DispatchError> {
			self.checked_fn(
				other,
				<BalanceOf<T> as CheckedAdd>::checked_add,
				ArithmeticError::Overflow,
			)
		}

		pub fn checked_sub(&self, other: &Self) -> Result<Self, DispatchError> {
			self.checked_fn(
				other,
				<BalanceOf<T> as CheckedSub>::checked_sub,
				ArithmeticError::Underflow,
			)
		}

		pub fn saturating_sub(&self, other: &Self) -> Result<Self, DispatchError> {
			ensure!(self.currency_id == other.currency_id, Error::<T>::InvalidCurrency);
			self.checked_sub(other)
				.or_else(|_| Ok(Self::new(0u32.into(), self.currency_id)))
		}

		pub fn checked_fixed_point_mul(
			&self,
			scalar: &UnsignedFixedPoint<T>,
		) -> Result<Self, DispatchError> {
			let amount = scalar.checked_mul_int(self.amount).ok_or(ArithmeticError::Underflow)?;
			Ok(Self { amount, currency_id: self.currency_id })
		}

		pub fn checked_fixed_point_mul_rounded_up(
			&self,
			scalar: &UnsignedFixedPoint<T>,
		) -> Result<Self, DispatchError> {
			let self_fixed_point = UnsignedFixedPoint::<T>::checked_from_integer(self.amount)
				.ok_or(Error::<T>::TryIntoIntError)?;

			log::info!("WHAT DA FAAAAAAACXKKKK PALLET-CURRENCY: SELF_FIXED_POINT: {:?} ",self_fixed_point);

			// do the multiplication
			let product = self_fixed_point.checked_mul(scalar).ok_or(ArithmeticError::Overflow)?;

			log::info!("WHAT DA FAAAAAAACXKKKK PALLET-CURRENCY: PRODUCT: {:?} ",product);


			// convert to inner
			let product_inner =
				UniqueSaturatedInto::<u128>::unique_saturated_into(product.into_inner());

			log::info!("WHAT DA FAAAAAAACXKKKK PALLET-CURRENCY: PRODUCT TO U128 {}",product_inner);

			// convert to u128 by dividing by a rounded up division by accuracy
			let accuracy = UniqueSaturatedInto::<u128>::unique_saturated_into(
				UnsignedFixedPoint::<T>::accuracy(),
			);

			let amount = product_inner
				.checked_add(accuracy)
				.ok_or(ArithmeticError::Overflow)?
				.checked_sub(1)
				.ok_or(ArithmeticError::Underflow)?
				.checked_div(accuracy)
				.ok_or(ArithmeticError::Underflow)?
				.try_into()
				.map_err(|_| Error::<T>::TryIntoIntError)?;

			log::info!("WHAT DA FAAAAAAACXKKKK PALLET-CURRENCY: new amount {:?}", amount);


			Ok(Self { amount, currency_id: self.currency_id })
		}

		pub fn checked_div(&self, scalar: &UnsignedFixedPoint<T>) -> Result<Self, DispatchError> {
			let amount = UnsignedFixedPoint::<T>::checked_from_integer(self.amount)
				.ok_or(Error::<T>::TryIntoIntError)?
				.checked_div(scalar)
				.ok_or(ArithmeticError::Overflow)?
				.truncate_to_inner()
				.ok_or(Error::<T>::TryIntoIntError)?;
			Ok(Self { amount, currency_id: self.currency_id })
		}

		pub fn ratio(&self, other: &Self) -> Result<UnsignedFixedPoint<T>, DispatchError> {
			ensure!(self.currency_id == other.currency_id, Error::<T>::InvalidCurrency);
			let ratio = UnsignedFixedPoint::<T>::checked_from_rational(self.amount, other.amount)
				.ok_or(Error::<T>::TryIntoIntError)?;
			Ok(ratio)
		}

		pub fn min(&self, other: &Self) -> Result<Self, DispatchError> {
			ensure!(self.currency_id == other.currency_id, Error::<T>::InvalidCurrency);
			Ok(if self.le(other)? { self.clone() } else { other.clone() })
		}

		pub fn lt(&self, other: &Self) -> Result<bool, DispatchError> {
			ensure!(self.currency_id == other.currency_id, Error::<T>::InvalidCurrency);
			Ok(self.amount < other.amount)
		}

		pub fn le(&self, other: &Self) -> Result<bool, DispatchError> {
			ensure!(self.currency_id == other.currency_id, Error::<T>::InvalidCurrency);
			Ok(self.amount <= other.amount)
		}

		pub fn eq(&self, other: &Self) -> Result<bool, DispatchError> {
			ensure!(self.currency_id == other.currency_id, Error::<T>::InvalidCurrency);
			Ok(self.amount == other.amount)
		}

		pub fn ne(&self, other: &Self) -> Result<bool, DispatchError> {
			Ok(!self.eq(other)?)
		}

		pub fn ge(&self, other: &Self) -> Result<bool, DispatchError> {
			ensure!(self.currency_id == other.currency_id, Error::<T>::InvalidCurrency);
			Ok(self.amount >= other.amount)
		}

		pub fn gt(&self, other: &Self) -> Result<bool, DispatchError> {
			ensure!(self.currency_id == other.currency_id, Error::<T>::InvalidCurrency);
			Ok(self.amount > other.amount)
		}

		pub fn rounded_mul(&self, fraction: UnsignedFixedPoint<T>) -> Result<Self, DispatchError> {
			// we add 0.5 before we do the final integer division to round the result we return.
			// note that unwrapping is safe because we use a constant
			let rounding_addition = UnsignedFixedPoint::<T>::checked_from_rational(1, 2)
				.ok_or(Error::<T>::TryIntoIntError)?;

			let amount = UnsignedFixedPoint::<T>::checked_from_integer(self.amount)
				.ok_or(ArithmeticError::Overflow)?
				.checked_mul(&fraction)
				.ok_or(ArithmeticError::Overflow)?
				.checked_add(&rounding_addition)
				.ok_or(ArithmeticError::Overflow)?
				.truncate_to_inner()
				.ok_or(Error::<T>::TryIntoIntError)?;

			Ok(Self { amount, currency_id: self.currency_id })
		}
	}
}

#[cfg_attr(feature = "testing-utils", mocktopus::macros::mockable)]
mod actions {
	use super::*;

	impl<T: Config> Amount<T> {
		pub fn transfer(
			&self,
			source: &AccountIdOf<T>,
			destination: &AccountIdOf<T>,
		) -> Result<(), DispatchError> {
			<orml_currencies::Pallet<T> as MultiCurrency<AccountIdOf<T>>>::transfer(
				self.currency_id,
				source,
				destination,
				self.amount,
			)?;
			Ok(())
		}

		pub fn lock_on(&self, account_id: &AccountIdOf<T>) -> Result<(), DispatchError> {
			<orml_currencies::Pallet<T>>::reserve(self.currency_id, account_id, self.amount)?;
			Ok(())
		}

		pub fn unlock_on(&self, account_id: &AccountIdOf<T>) -> Result<(), DispatchError> {
			ensure!(
				<orml_currencies::Pallet<T>>::unreserve(self.currency_id, account_id, self.amount)
					.is_zero(),
				orml_currencies::Error::<T>::BalanceTooLow
			);
			Ok(())
		}

		pub fn burn_from(&self, account_id: &AccountIdOf<T>) -> DispatchResult {
			ensure!(
				<orml_currencies::Pallet<T>>::slash_reserved(
					self.currency_id,
					account_id,
					self.amount
				)
				.is_zero(),
				orml_currencies::Error::<T>::BalanceTooLow
			);
			Ok(())
		}

		pub fn mint_to(&self, account_id: &AccountIdOf<T>) -> DispatchResult {
			<orml_currencies::Pallet<T>>::deposit(self.currency_id, account_id, self.amount)?;
			Ok(())
		}

		pub fn map<F: Fn(BalanceOf<T>) -> BalanceOf<T>>(&self, f: F) -> Self {
			Amount::new(f(self.amount), self.currency_id)
		}
	}
}

#[cfg(feature = "testing-utils")]
mod testing_utils {
	use sp_std::{
		cmp::{Ordering, PartialOrd},
		ops::{Add, AddAssign, Div, Mul, Sub, SubAssign},
	};

	use super::*;

	impl<T: Config> Amount<T> {
		pub fn with_amount<F: FnOnce(BalanceOf<T>) -> BalanceOf<T>>(&self, f: F) -> Self {
			Self { amount: f(self.amount), currency_id: self.currency_id }
		}
	}
	impl<T: Config> AddAssign for Amount<T> {
		fn add_assign(&mut self, other: Self) {
			*self = self.clone() + other;
		}
	}

	impl<T: Config> SubAssign for Amount<T> {
		fn sub_assign(&mut self, other: Self) {
			*self = self.clone() - other;
		}
	}

	impl<T: Config> Add<Self> for Amount<T> {
		type Output = Self;

		fn add(self, other: Self) -> Self {
			if self.currency_id != other.currency_id {
				panic!("Adding two different currencies")
			}
			Self { amount: self.amount + other.amount, currency_id: self.currency_id }
		}
	}

	impl<T: Config> Sub for Amount<T> {
		type Output = Self;

		fn sub(self, other: Self) -> Self {
			if self.currency_id != other.currency_id {
				panic!("Subtracting two different currencies")
			}
			Self { amount: self.amount - other.amount, currency_id: self.currency_id }
		}
	}

	impl<T: Config> Mul<BalanceOf<T>> for Amount<T> {
		type Output = Self;

		fn mul(self, other: BalanceOf<T>) -> Self {
			Self { amount: self.amount * other, currency_id: self.currency_id }
		}
	}

	impl<T: Config> Div<BalanceOf<T>> for Amount<T> {
		type Output = Self;

		fn div(self, other: BalanceOf<T>) -> Self {
			Self { amount: self.amount / other, currency_id: self.currency_id }
		}
	}

	impl<T: Config> PartialOrd for Amount<T> {
		fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
			if self.currency_id != other.currency_id {
				None
			} else {
				Some(self.amount.cmp(&other.amount))
			}
		}
	}
}
