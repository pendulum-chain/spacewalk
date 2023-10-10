use crate::Config;
use frame_support::traits::Currency;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

pub(crate) type BalanceOf<T> = <<T as Config>::Currency as Currency<AccountIdOf<T>>>::Balance;
