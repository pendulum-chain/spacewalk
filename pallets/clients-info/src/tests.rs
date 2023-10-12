use crate::mock::*;
use sp_core::H256;
use sp_std::vec;
use std::collections::HashMap;
use upgrade_client_releases::*;

#[cfg(test)]
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

#[cfg(test)]
#[test]
fn test_decode_bounded_vec() {
	run_test(|| {
		let key = vec![0; 255];

		CurrentClientReleases::<Test>::insert(
			key.clone(),
			ClientRelease { uri: vec![1; 255], checksum: H256::default() },
		);

		let client_release =
			crate::CurrentClientReleases::<Test>::get(BoundedVec::try_from(key).unwrap());
		assert_eq!(client_release.map(|c| c.uri.to_vec()), Some(vec![1; 255]));
	});
use crate::{
	mock::*,
	pallet::{CurrentClientReleases, PendingClientReleases},
	upgrade_client_releases::*,
	ClientRelease, UriOf,
};
use frame_support::BoundedVec;
use sp_core::H256;
use sp_std::vec;

#[cfg(test)]
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
}
