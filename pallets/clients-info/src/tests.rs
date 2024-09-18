#![deny(warnings)]
use crate::{
	mock::{run_test, ClientsInfo, RuntimeOrigin, Test, TestError},
	pallet::{AuthorizedAccounts, CurrentClientReleases, PendingClientReleases},
	upgrade_client_releases::try_upgrade_current_client_releases,
	ClientRelease,
};
use frame_support::{assert_err, BoundedVec, sp_runtime::testing::H256 };
use std::collections::HashMap;

#[test]
fn test_client_pending_release_migration() {
	run_test(|| {
		let vault_key = b"vault".to_vec();

		let pre_migration_pending_releases: HashMap<_, _> = vec![(
			vault_key.clone(),
			ClientRelease {
				uri: BoundedVec::try_from(b"https://github.com/pendulum-chain/spacewalk".to_vec())
					.unwrap(),
				checksum: H256::default(),
			},
		)]
		.into_iter()
		.collect();
		pre_migration_pending_releases.iter().for_each(|(key, value)| {
			PendingClientReleases::<Test>::insert(
				BoundedVec::try_from(key.clone()).unwrap(),
				value.clone(),
			);
		});

		let pre_migration_current_releases: HashMap<_, _> = vec![(
			vault_key.clone(),
			ClientRelease {
				uri: BoundedVec::try_from(b"https://github.com/pendulum-chain/spacewalk".to_vec())
					.unwrap(),
				checksum: H256::default(),
			},
		)]
		.into_iter()
		.collect();
		pre_migration_current_releases.iter().for_each(|(key, value)| {
			CurrentClientReleases::<Test>::insert(
				BoundedVec::try_from(key.clone()).unwrap(),
				value.clone(),
			);
		});

		try_upgrade_current_client_releases::<Test>();

		let pending_releases = PendingClientReleases::<Test>::iter_values().collect::<Vec<_>>();
		assert_eq!(pending_releases.is_empty(), true);

		let current_releases = CurrentClientReleases::<Test>::iter()
			.map(|(key, value)| (key.to_vec(), value))
			.collect::<HashMap<_, _>>();
		assert_eq!(
			current_releases.get(&vault_key),
			pre_migration_pending_releases.get(&vault_key)
		);
	});
}

#[test]
fn authorize_account_should_work() {
	run_test(|| {
		<AuthorizedAccounts<Test>>::insert(1, ());

		let _test1 = ClientsInfo::authorize_account(RuntimeOrigin::signed(1), 2);
		let _test2 = ClientsInfo::authorize_account(RuntimeOrigin::signed(1), 3);
		let _test3 = ClientsInfo::authorize_account(RuntimeOrigin::signed(1), 4);
		assert_eq!(<AuthorizedAccounts<Test>>::contains_key(2), true);
		assert_eq!(<AuthorizedAccounts<Test>>::contains_key(3), true);
		assert_eq!(<AuthorizedAccounts<Test>>::contains_key(4), true);

		let client_release = ClientRelease {
			uri: BoundedVec::try_from(b"https://github.com/pendulum-chain/spacewalk".to_vec())
				.unwrap(),
			checksum: H256::default(),
		};

		let client_name = BoundedVec::try_from(b"vault".to_vec()).expect("should work");

		assert!(ClientsInfo::set_current_client_release(
			RuntimeOrigin::signed(2),
			client_name,
			client_release,
		)
		.is_ok());
	})
}

#[test]
fn deauthorize_account_should_work() {
	run_test(|| {
		<AuthorizedAccounts<Test>>::insert(1, ());
		<AuthorizedAccounts<Test>>::insert(2, ());
		<AuthorizedAccounts<Test>>::insert(3, ());

		assert!(ClientsInfo::deauthorize_account(RuntimeOrigin::root(), 1).is_ok());
		assert!(ClientsInfo::deauthorize_account(RuntimeOrigin::signed(2), 3).is_ok());

		assert_eq!(<AuthorizedAccounts<Test>>::contains_key(1), false);
		assert_eq!(<AuthorizedAccounts<Test>>::contains_key(2), true);
		assert_eq!(<AuthorizedAccounts<Test>>::contains_key(3), false);
	})
}

#[test]
fn test_errors() {
	run_test(|| {
		<AuthorizedAccounts<Test>>::insert(1, ());
		<AuthorizedAccounts<Test>>::insert(2, ());
		<AuthorizedAccounts<Test>>::insert(3, ());

		assert_err!(
			ClientsInfo::authorize_account(RuntimeOrigin::signed(4), 4),
			TestError::ThisAccountIdIsNotAuthorized
		);

		assert_err!(
			ClientsInfo::authorize_account(RuntimeOrigin::signed(4), 3),
			TestError::ThisAccountIdIsNotAuthorized
		);

		assert_err!(
			ClientsInfo::deauthorize_account(RuntimeOrigin::signed(1), 1),
			TestError::UserUnableToDeauthorizeThemself
		);

		assert_err!(
			ClientsInfo::deauthorize_account(RuntimeOrigin::signed(4), 1),
			TestError::ThisAccountIdIsNotAuthorized
		);
	})
}

#[test]
fn test_decode_bounded_vec() {
	run_test(|| {
		let key = vec![0u8; 255];

		CurrentClientReleases::<Test>::insert(
			BoundedVec::try_from(key.clone()).expect("should create a key"),
			ClientRelease {
				uri: BoundedVec::try_from(vec![1; 255]).expect("should create uri"),
				checksum: H256::default(),
			},
		);

		let client_release = CurrentClientReleases::<Test>::get(BoundedVec::try_from(key).unwrap());
		assert_eq!(client_release.map(|c| c.uri.to_vec()), Some(vec![1; 255]));
	});
}
