use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use orml_traits::MultiCurrency;
use sp_core::{Get, H256};
use sp_runtime::{traits::One, FixedPointNumber};
use sp_std::prelude::*;

use currency::{
	getters::{get_relay_chain_currency_id as get_collateral_currency_id, *},
	testing_constants::get_wrapped_currency_id,
};
use oracle::Pallet as Oracle;
use primitives::{CurrencyId, VaultCurrencyPair, VaultId};
use security::Pallet as Security;
use stellar_relay::{
	testing_utils::{
		build_dummy_proof_for, get_validators_and_organizations, DEFAULT_STELLAR_PUBLIC_KEY,
	},
	Config as StellarRelayConfig, Pallet as StellarRelay,
};
use vault_registry::{types::DefaultVaultCurrencyPair, Pallet as VaultRegistry};

// Pallets
use crate::Pallet as Issue;

use super::*;

fn deposit_tokens<T: crate::Config>(
	currency_id: CurrencyId,
	account_id: &T::AccountId,
	amount: BalanceOf<T>,
) {
	assert_ok!(<orml_tokens::Pallet<T>>::deposit(currency_id, account_id, amount));
}

fn mint_collateral<T: crate::Config>(account_id: &T::AccountId, amount: BalanceOf<T>) {
	deposit_tokens::<T>(get_collateral_currency_id::<T>(), account_id, amount);
	deposit_tokens::<T>(get_native_currency_id::<T>(), account_id, amount);
}

fn get_currency_pair<T: crate::Config>() -> DefaultVaultCurrencyPair<T> {
	VaultCurrencyPair {
		collateral: get_collateral_currency_id::<T>(),
		wrapped: get_wrapped_currency_id(),
	}
}

fn get_vault_id<T: crate::Config>() -> DefaultVaultId<T> {
	VaultId::new(
		account("Vault", 0, 0),
		get_collateral_currency_id::<T>(),
		get_wrapped_currency_id(),
	)
}

fn register_vault<T: crate::Config>(vault_id: DefaultVaultId<T>) {
	let origin = RawOrigin::Signed(vault_id.account_id.clone());
	assert_ok!(VaultRegistry::<T>::register_public_key(origin.into(), DEFAULT_STELLAR_PUBLIC_KEY));
	assert_ok!(VaultRegistry::<T>::_register_vault(vault_id, 100000000u32.into()));
}

benchmarks! {
	request_issue {
		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();
		let amount = 1000u32.into();
		let asset = vault_id.wrapped_currency();
		let relayer_id: T::AccountId = account("Relayer", 0, 0);

		Oracle::<T>::_set_exchange_rate(origin.clone(), get_collateral_currency_id::<T>(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();
		Oracle::<T>::_set_exchange_rate(origin.clone(), get_wrapped_currency_id(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();
		Oracle::<T>::_set_exchange_rate(origin.clone(), <T as vault_registry::Config>::GetGriefingCollateralCurrencyId::get(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();

		mint_collateral::<T>(&origin, (1u32 << 31).into());
		mint_collateral::<T>(&vault_id.account_id, (1u32 << 31).into());
		mint_collateral::<T>(&relayer_id, (1u32 << 31).into());

		VaultRegistry::<T>::_set_secure_collateral_threshold(get_currency_pair::<T>(), <T as currency::Config>::UnsignedFixedPoint::checked_from_rational(1, 100000).unwrap());// 0.001%
		VaultRegistry::<T>::_set_system_collateral_ceiling(get_currency_pair::<T>(), 1_000_000_000u32.into());
		register_vault::<T>(vault_id.clone());

		Security::<T>::set_active_block_number(1u32.into());
	}: _(RawOrigin::Signed(origin), amount, vault_id)

	execute_issue {
		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();
		let relayer_id: T::AccountId = account("Relayer", 0, 0);

		mint_collateral::<T>(&origin, (1u32 << 31).into());
		mint_collateral::<T>(&vault_id.account_id, (1u32 << 31).into());
		mint_collateral::<T>(&relayer_id, (1u32 << 31).into());

		let vault_stellar_address = DEFAULT_STELLAR_PUBLIC_KEY;
		let value: Amount<T> = Amount::new(2u32.into(), get_wrapped_currency_id());

		let issue_id = H256::zero();
		let issue_request = IssueRequest {
			requester: origin.clone(),
			vault: vault_id.clone(),
			amount: value.amount(),
			asset: value.currency(),
			stellar_address: vault_stellar_address,
			fee: Default::default(),
			griefing_collateral: Default::default(),
			opentime: Default::default(),
			period: Default::default(),
			status: Default::default(),
		};
		Issue::<T>::insert_issue_request(&issue_id, &issue_request);
		Security::<T>::set_active_block_number(1u32.into());

		let (validators, organizations) = get_validators_and_organizations::<T>();
		let enactment_block_height = T::BlockNumber::default();
		StellarRelay::<T>::_update_tier_1_validator_set(validators, organizations, enactment_block_height).unwrap();
		let public_network = <T as StellarRelayConfig>::IsPublicNetwork::get();
		let (tx_env_xdr_encoded, scp_envs_xdr_encoded, tx_set_xdr_encoded) = build_dummy_proof_for::<T>(issue_id, public_network);

		VaultRegistry::<T>::_set_system_collateral_ceiling(get_currency_pair::<T>(), 1_000_000_000u32.into());
		VaultRegistry::<T>::_set_secure_collateral_threshold(get_currency_pair::<T>(), <T as currency::Config>::UnsignedFixedPoint::checked_from_rational(1, 100000).unwrap());
		Oracle::<T>::_set_exchange_rate(origin.clone(), get_collateral_currency_id::<T>(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();
		Oracle::<T>::_set_exchange_rate(origin.clone(), get_wrapped_currency_id(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();
		register_vault::<T>(vault_id.clone());

		VaultRegistry::<T>::try_increase_to_be_issued_tokens(&vault_id, &value).unwrap();
		let secure_id = Security::<T>::get_secure_id(&vault_id.account_id);
		VaultRegistry::<T>::register_deposit_address(&vault_id, secure_id).unwrap();
	}: _(RawOrigin::Signed(origin), issue_id, tx_env_xdr_encoded, scp_envs_xdr_encoded, tx_set_xdr_encoded)

	cancel_issue {
		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();

		mint_collateral::<T>(&origin, (1u32 << 31).into());
		mint_collateral::<T>(&vault_id.account_id, (1u32 << 31).into());

		let vault_stellar_address = DEFAULT_STELLAR_PUBLIC_KEY;
		let value = Amount::new(2u32.into(), get_wrapped_currency_id());

		let issue_id = H256::zero();
		let issue_request = IssueRequest {
			requester: origin.clone(),
			vault: vault_id.clone(),
			stellar_address: vault_stellar_address,
			amount: value.amount(),
			asset: value.currency(),
			opentime: Security::<T>::active_block_number(),
			fee: Default::default(),
			griefing_collateral: Default::default(),
			period: Default::default(),
			status: Default::default(),
		};

		Issue::<T>::insert_issue_request(&issue_id, &issue_request);

		// expire issue request
		Security::<T>::set_active_block_number(issue_request.opentime + Issue::<T>::issue_period() + 100u32.into());

		VaultRegistry::<T>::_set_system_collateral_ceiling(get_currency_pair::<T>(), 1_000_000_000u32.into());
		VaultRegistry::<T>::_set_secure_collateral_threshold(get_currency_pair::<T>(), <T as currency::Config>::UnsignedFixedPoint::checked_from_rational(1, 100000).unwrap());
		Oracle::<T>::_set_exchange_rate(origin.clone(), get_collateral_currency_id::<T>(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();
		Oracle::<T>::_set_exchange_rate(origin.clone(), get_wrapped_currency_id(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();
		register_vault::<T>(vault_id.clone());

		VaultRegistry::<T>::try_increase_to_be_issued_tokens(&vault_id, &value).unwrap();
		let secure_id = Security::<T>::get_secure_id(&vault_id.account_id);
		VaultRegistry::<T>::register_deposit_address(&vault_id, secure_id).unwrap();

	}: _(RawOrigin::Signed(origin), issue_id)

	set_issue_period {
	}: _(RawOrigin::Root, 1u32.into())

	rate_limit_update {
		let limit_volume_amount: Option<BalanceOf<T>> = Some(1u32.into());
		let limit_volume_currency_id: T::CurrencyId = get_wrapped_currency_id();
		let interval_length: T::BlockNumber = 1u32.into();
	}: _(RawOrigin::Root, limit_volume_amount, limit_volume_currency_id, interval_length)

	minimum_transfer_amount_update {
		let new_minimum_amount: BalanceOf<T> = 1u32.into();
	}: _(RawOrigin::Root, new_minimum_amount)
}

impl_benchmark_test_suite!(
	Issue,
	crate::mock::ExtBuilder::build_with(Default::default()),
	crate::mock::Test
);
