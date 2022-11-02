use currency::CurrencyId;
use primitives::VaultId;

use crate::Config;

pub(crate) type BalanceOf<T> = <T as currency::Config>::Balance;

pub(crate) type SignedFixedPoint<T> = <T as Config>::SignedFixedPoint;

pub(crate) type UnsignedFixedPoint<T> = <T as Config>::UnsignedFixedPoint;

pub(crate) type DefaultVaultId<T> = VaultId<<T as frame_system::Config>::AccountId, CurrencyId<T>>;
