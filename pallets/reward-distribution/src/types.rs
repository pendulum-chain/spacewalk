use crate::Config;
use currency::CurrencyId;
use primitives::VaultId;

pub(crate) type BalanceOf<T> = <T as Config>::Balance;

pub(crate) type DefaultVaultId<T> = VaultId<<T as frame_system::Config>::AccountId, CurrencyId<T>>;
