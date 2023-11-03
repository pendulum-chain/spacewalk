use super::*;
use frame_benchmarking::v2::{benchmarks, impl_benchmark_test_suite, Linear};
use frame_system::RawOrigin;
use sp_std::vec;

#[allow(unused)]
use super::Pallet as ClientsInfo;

#[benchmarks]
pub mod benchmarks {
	use super::*;
	use frame_benchmarking::{account, whitelisted_caller};

	#[benchmark]
	fn set_current_client_release(n: Linear<0, 255>, u: Linear<0, 255>) {
		let caller: T::AccountId = whitelisted_caller();
		AuthorizedAccounts::<T>::insert(caller.clone(), ());

		let name = BoundedVec::try_from(vec![0; n as usize]).unwrap();
		let uri = BoundedVec::try_from(vec![0; u as usize]).unwrap();
		let client_release = ClientRelease { uri, checksum: Default::default() };

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), name.clone(), client_release.clone());

		assert_eq!(CurrentClientReleases::<T>::get(name), Some(client_release));
	}

	#[benchmark]
	fn set_pending_client_release(n: Linear<0, 255>, u: Linear<0, 255>) {
		let caller: T::AccountId = whitelisted_caller();
		AuthorizedAccounts::<T>::insert(caller.clone(), ());

		let name = BoundedVec::try_from(vec![0; n as usize]).unwrap();
		let uri = BoundedVec::try_from(vec![0; u as usize]).unwrap();
		let client_release = ClientRelease { uri, checksum: Default::default() };

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), name.clone(), client_release.clone());

		assert_eq!(PendingClientReleases::<T>::get(name), Some(client_release));
	}

	#[benchmark]
	fn authorize_account() {
		let caller: T::AccountId = whitelisted_caller();
		let account: T::AccountId = account("test", 2, 2);

		AuthorizedAccounts::<T>::insert(caller.clone(), ());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), account.clone());

		assert!(AuthorizedAccounts::<T>::get(account).is_some());
	}

	#[benchmark]
	fn deauthorize_account() {
		let caller: T::AccountId = whitelisted_caller();
		let account: T::AccountId = account("test", 2, 2);

		AuthorizedAccounts::<T>::insert(caller.clone(), ());
		AuthorizedAccounts::<T>::insert(account.clone(), ());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), account.clone());

		assert!(AuthorizedAccounts::<T>::get(account).is_none());
	}

	impl_benchmark_test_suite!(ClientsInfo, crate::mock::ExtBuilder::build(), crate::mock::Test);
}
