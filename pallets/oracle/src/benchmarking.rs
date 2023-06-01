#![allow(warnings)]
use super::{Pallet as Oracle, *};
use crate::OracleKey;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_std::prelude::*;

use pallet_timestamp::Pallet as Timestamp;

benchmarks! {
	on_initialize {}: {
		Timestamp::<T>::set_timestamp(1000u32.into());
	}

	update_oracle_keys {
		let v: Vec<OracleKey> = vec![OracleKey::ExchangeRate(CurrencyId::Native)];
	}: _(RawOrigin::Root, v)
	verify {
		let v : Vec<OracleKey> = vec![OracleKey::ExchangeRate(CurrencyId::Native)];
		assert_eq!(OracleKeys::<T>::get(), v);
	}

	set_max_delay {
		let new_max_delay: T::Moment = 1000u32.into();
	}: _(RawOrigin::Root, new_max_delay)
	verify {
		let new_max_delay: T::Moment = 1000u32.into();
		assert_eq!(MaxDelay::<T>::get(), new_max_delay);
	}
}

impl_benchmark_test_suite!(Oracle, crate::mock::ExtBuilder::build(), crate::mock::Test);
