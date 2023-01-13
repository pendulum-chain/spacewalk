use super::{Pallet as Oracle, *};
use crate::OracleKey;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use primitives::{CurrencyId::Token, TokenSymbol::*};
use sp_runtime::FixedPointNumber;
use sp_std::prelude::*;

use pallet_timestamp::Pallet as Timestamp;

type MomentOf<T> = <T as pallet_timestamp::Config>::Moment;

benchmarks! {
	on_initialize {}: {
		RawValuesUpdated::<T>::insert(OracleKey::ExchangeRate(Token(DOT)), true);

		let valid_until: MomentOf<T> = 100u32.into();
		ValidUntil::<T>::insert(OracleKey::ExchangeRate(Token(DOT)), valid_until);

		Timestamp::<T>::set_timestamp(1000u32.into());
	}
}

impl_benchmark_test_suite!(Oracle, crate::mock::ExtBuilder::build(), crate::mock::Test);
