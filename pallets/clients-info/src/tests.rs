#![deny(warnings)]
use crate::{
	mock::{run_test, Test},
	pallet::{CurrentClientReleases, PendingClientReleases},
	upgrade_client_releases::try_upgrade_current_client_releases,
	ClientRelease,
};
use frame_support::BoundedVec;
use sp_runtime::testing::H256;
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

fn test_authorize_accounts() {
	run_test(|| {})
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
