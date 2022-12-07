use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

use currency::getters::get_native_currency_id;
use primitives::VaultId;

#[cfg(test)]
use crate::Pallet as Fee;

use super::*;

const SEED: u32 = 0;

fn get_fee<T: crate::Config>() -> UnsignedFixedPoint<T> {
	let fee: UnsignedFixedPoint<T> =
		UnsignedFixedPoint::<T>::saturating_from_rational(1u32, 100u32);
	fee
}

benchmarks! {
	withdraw_rewards {
		let nominator: T::AccountId = account("recipient", 0, SEED);
		let vault_id: VaultId<_, CurrencyId<T>> = VaultId::new(nominator.clone(), get_native_currency_id::<T>(), get_native_currency_id::<T>());
	}: _(RawOrigin::Signed(nominator), vault_id, None)

	set_issue_fee {
		let caller: T::AccountId = account("caller", 0, SEED);
		let fee = get_fee::<T>();
	}: _(RawOrigin::Root, fee)

	set_issue_griefing_collateral {
		let caller: T::AccountId = account("caller", 0, SEED);
		let fee = get_fee::<T>();
	}: _(RawOrigin::Root, fee)

	set_redeem_fee {
		let caller: T::AccountId = account("caller", 0, SEED);
		let fee = get_fee::<T>();
	}: _(RawOrigin::Root, fee)

	set_premium_redeem_fee {
		let caller: T::AccountId = account("caller", 0, SEED);
		let fee = get_fee::<T>();
	}: _(RawOrigin::Root, fee)

	set_punishment_fee {
		let caller: T::AccountId = account("caller", 0, SEED);
		let fee = get_fee::<T>();
	}: _(RawOrigin::Root, fee)

	set_replace_griefing_collateral {
		let caller: T::AccountId = account("caller", 0, SEED);
		let fee = get_fee::<T>();
	}: _(RawOrigin::Root, fee)
}

impl_benchmark_test_suite!(Fee, crate::mock::ExtBuilder::build(), crate::mock::Test);
