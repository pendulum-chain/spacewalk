use super::*;
use frame_benchmarking::v2::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_std::vec;

#[allow(unused)]
use super::Pallet as RewardDistribution;

#[benchmarks]
pub mod benchmarks {
	use super::*;

	#[benchmark]
	fn set_reward_per_block() {
		let new_reward_per_block = Default::default();

		#[extrinsic_call]
		_(RawOrigin::Root, new_reward_per_block);

		assert_eq!(RewardDistribution::<T>::reward_per_block(), Some(new_reward_per_block));
	}

	impl_benchmark_test_suite!(
		RewardDistribution,
		crate::mock::ExtBuilder::build(),
		crate::mock::Test
	);
}
