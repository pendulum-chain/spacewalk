use crate::Config;
//use currency::CurrencyId;
use primitives::{CurrencyId, VaultId};

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

pub(crate) type BalanceOf<T> = <T as Config>::Balance;

pub(crate) type DefaultVaultId<T> = VaultId<<T as frame_system::Config>::AccountId, CurrencyId>;
