#![allow(warnings)]
use super::{Pallet as Oracle, *};
use crate::OracleKey;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_std::prelude::*;

use pallet_timestamp::Pallet as Timestamp;

benchmarks! {
	on_initialize {}: {
		Timestamp::<T>::set_timestamp(1000u32.into());
	}

	update_oracle_keys {
		let origin: T::AccountId = account("origin", 0, 0);
		let v : Vec<OracleKey> = vec![OracleKey::ExchangeRate(CurrencyId::Native)];
	}: _(RawOrigin::Root, v)
	verify {
		let v : Vec<OracleKey> = vec![OracleKey::ExchangeRate(CurrencyId::Native)];
		assert_eq!(OracleKeys::<T>::get(), v);
	}
}

impl_benchmark_test_suite!(Oracle, crate::mock::ExtBuilder::build(), crate::mock::Test);
