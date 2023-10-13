use crate::Config;
use primitives::{CurrencyId, VaultId};

pub(crate) type BalanceOf<T> = <T as Config>::Balance;

pub(crate) type DefaultVaultId<T> = VaultId<<T as frame_system::Config>::AccountId, CurrencyId>;
