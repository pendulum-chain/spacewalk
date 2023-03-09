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
use oracle::{OracleKey, Pallet as Oracle};
use primitives::{CurrencyId, VaultCurrencyPair, VaultId};
use security::Pallet as Security;
use stellar_relay::{
	testing_utils::{
		build_dummy_proof_for, get_validators_and_organizations, DEFAULT_STELLAR_PUBLIC_KEY,
		RANDOM_STELLAR_PUBLIC_KEY,
	},
	Config as StellarRelayConfig, Pallet as StellarRelay,
};
use vault_registry::{
	types::{DefaultVaultCurrencyPair, Vault},
	Pallet as VaultRegistry,
};

// Pallets
use crate::Pallet as Replace;

use super::*;

type UnsignedFixedPoint<T> = <T as currency::Config>::UnsignedFixedPoint;

fn wrapped<T: crate::Config>(amount: u32) -> Amount<T> {
	Amount::new(amount.into(), get_wrapped_currency_id())
}

fn get_currency_pair<T: crate::Config>() -> DefaultVaultCurrencyPair<T> {
	VaultCurrencyPair {
		collateral: get_collateral_currency_id::<T>(),
		wrapped: get_wrapped_currency_id(),
	}
}

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

fn initialize_oracle<T: crate::Config>() {
	let oracle_id: T::AccountId = account("Oracle", 12, 0);

	let result = Oracle::<T>::_feed_values(
		oracle_id,
		vec![
			(
				OracleKey::ExchangeRate(get_collateral_currency_id::<T>()),
				UnsignedFixedPoint::<T>::checked_from_rational(1, 1).unwrap(),
			),
			(
				OracleKey::ExchangeRate(get_native_currency_id::<T>()),
				UnsignedFixedPoint::<T>::checked_from_rational(1, 1).unwrap(),
			),
			(
				OracleKey::ExchangeRate(get_wrapped_currency_id()),
				UnsignedFixedPoint::<T>::checked_from_rational(1, 1).unwrap(),
			),
		],
	);
	assert_ok!(result);
	Oracle::<T>::begin_block(0u32.into());
}

fn test_request<T: crate::Config>(
	new_vault_id: &DefaultVaultId<T>,
	old_vault_id: &DefaultVaultId<T>,
) -> DefaultReplaceRequest<T> {
	ReplaceRequest {
		new_vault: new_vault_id.clone(),
		old_vault: old_vault_id.clone(),
		period: Default::default(),
		accept_time: Default::default(),
		amount: Default::default(),
		asset: get_wrapped_currency_id(),
		griefing_collateral: Default::default(),
		collateral: Default::default(),
		status: Default::default(),
		stellar_address: Default::default(),
	}
}

fn get_vault_id<T: crate::Config>(name: &'static str) -> DefaultVaultId<T> {
	VaultId::new(account(name, 0, 0), get_collateral_currency_id::<T>(), get_wrapped_currency_id())
}

fn register_public_key<T: crate::Config>(vault_id: DefaultVaultId<T>) {
	let origin = RawOrigin::Signed(vault_id.account_id);
	assert_ok!(VaultRegistry::<T>::register_public_key(origin.into(), DEFAULT_STELLAR_PUBLIC_KEY));
}

fn register_vault<T: crate::Config>(vault_id: DefaultVaultId<T>) {
	register_public_key::<T>(vault_id.clone());
	assert_ok!(VaultRegistry::<T>::_register_vault(vault_id, 100000000u32.into()));
}

benchmarks! {
	request_replace {
		initialize_oracle::<T>();
		let vault_id = get_vault_id::<T>("Vault");
		mint_collateral::<T>(&vault_id.account_id, (1u32 << 31).into());
		let amount = Replace::<T>::minimum_transfer_amount(get_wrapped_currency_id()).amount() + 1000u32.into();

		register_public_key::<T>(vault_id.clone());

		let vault = Vault {
			id: vault_id.clone(),
			issued_tokens: amount,
			..Vault::new(vault_id.clone())
		};

		VaultRegistry::<T>::insert_vault(
			&vault_id,
			vault
		);

		VaultRegistry::<T>::_set_system_collateral_ceiling(vault_id.currencies.clone(), 1_000_000_000u32.into());
	}: _(RawOrigin::Signed(vault_id.account_id.clone()), vault_id.currencies.clone(), amount)

	withdraw_replace {
		initialize_oracle::<T>();
		let vault_id = get_vault_id::<T>("OldVault");
		mint_collateral::<T>(&vault_id.account_id, (1u32 << 31).into());
		let amount = wrapped(5);

		let threshold = UnsignedFixedPoint::<T>::one();
		VaultRegistry::<T>::_set_secure_collateral_threshold(get_currency_pair::<T>(), threshold);
		VaultRegistry::<T>::_set_system_collateral_ceiling(get_currency_pair::<T>(), 1_000_000_000u32.into());

		register_vault::<T>(vault_id.clone());

		VaultRegistry::<T>::try_increase_to_be_issued_tokens(&vault_id, &amount).unwrap();
		VaultRegistry::<T>::issue_tokens(&vault_id, &amount).unwrap();
		VaultRegistry::<T>::try_increase_to_be_replaced_tokens(&vault_id, &amount).unwrap();

		// TODO: check that an amount was actually withdrawn
	}: _(RawOrigin::Signed(vault_id.account_id.clone()), vault_id.currencies.clone(), amount.amount())

	accept_replace {
		initialize_oracle::<T>();
		let new_vault_id = get_vault_id::<T>("NewVault");
		let old_vault_id = get_vault_id::<T>("OldVault");
		mint_collateral::<T>(&old_vault_id.account_id, (1u32 << 31).into());
		mint_collateral::<T>(&new_vault_id.account_id, (1u32 << 31).into());
		let dust_value =  Replace::<T>::minimum_transfer_amount(get_wrapped_currency_id());
		let amount = dust_value.checked_add(&wrapped(100u32)).unwrap();
		let griefing = 1000u32.into();

		let new_vault_stellar_address = RANDOM_STELLAR_PUBLIC_KEY;

		VaultRegistry::<T>::_set_secure_collateral_threshold(get_currency_pair::<T>(), UnsignedFixedPoint::<T>::checked_from_rational(1, 100000).unwrap());
		VaultRegistry::<T>::_set_system_collateral_ceiling(get_currency_pair::<T>(), 1_000_000_000u32.into());
		register_vault::<T>(old_vault_id.clone());

		VaultRegistry::<T>::try_increase_to_be_issued_tokens(&old_vault_id, &amount).unwrap();
		VaultRegistry::<T>::issue_tokens(&old_vault_id, &amount).unwrap();
		VaultRegistry::<T>::try_increase_to_be_replaced_tokens(&old_vault_id, &amount).unwrap();

		register_vault::<T>(new_vault_id.clone());

		let replace_id = H256::zero();
		let mut replace_request = test_request::<T>(&old_vault_id, &old_vault_id);
		replace_request.amount = amount.amount();
		Replace::<T>::insert_replace_request(&replace_id, &replace_request);

	}: _(RawOrigin::Signed(new_vault_id.account_id.clone()), new_vault_id.currencies.clone(), old_vault_id, amount.amount(), griefing, new_vault_stellar_address)

	execute_replace {
		initialize_oracle::<T>();
		let new_vault_id = get_vault_id::<T>("NewVault");
		let old_vault_id = get_vault_id::<T>("OldVault");
		let relayer_id: T::AccountId = account("Relayer", 0, 0);

		let new_vault_stellar_address = [4u8; 32];
		let old_vault_stellar_address = [5u8; 32];

		let replace_id = H256::zero();
		let mut replace_request = test_request::<T>(&new_vault_id, &old_vault_id);
		replace_request.stellar_address = old_vault_stellar_address;

		Replace::<T>::insert_replace_request(&replace_id, &replace_request);

		let old_vault = Vault {
			id: old_vault_id.clone(),
			..Vault::new(old_vault_id.clone())
		};
		VaultRegistry::<T>::insert_vault(
			&old_vault_id,
			old_vault
		);

		let new_vault = Vault {
			id: new_vault_id.clone(),
			..Vault::new(new_vault_id.clone())
		};
		VaultRegistry::<T>::insert_vault(
			&new_vault_id,
			new_vault
		);

		Security::<T>::set_active_block_number(1u32.into());
		VaultRegistry::<T>::_set_system_collateral_ceiling(get_currency_pair::<T>(), 1_000_000_000u32.into());

		let (validators, organizations) = get_validators_and_organizations::<T>();
		let enactment_block_height = T::BlockNumber::default();
		StellarRelay::<T>::_update_tier_1_validator_set(validators, organizations, enactment_block_height).unwrap();
		let public_network = <T as StellarRelayConfig>::IsPublicNetwork::get();
		let (tx_env_xdr_encoded, scp_envs_xdr_encoded, tx_set_xdr_encoded) = build_dummy_proof_for::<T>(replace_id, public_network);

	}: _(RawOrigin::Signed(old_vault_id.account_id), replace_id, tx_env_xdr_encoded, scp_envs_xdr_encoded, tx_set_xdr_encoded)

	cancel_replace {
		initialize_oracle::<T>();
		let new_vault_id = get_vault_id::<T>("NewVault");
		let old_vault_id = get_vault_id::<T>("OldVault");
		mint_collateral::<T>(&new_vault_id.account_id, (1u32 << 31).into());
		mint_collateral::<T>(&old_vault_id.account_id, (1u32 << 31).into());

		let amount = wrapped(100);

		let replace_id = H256::zero();
		let mut replace_request = test_request::<T>(&new_vault_id, &old_vault_id);
		replace_request.amount = amount.amount();
		Replace::<T>::insert_replace_request(&replace_id, &replace_request);

		// expire replace request
		Security::<T>::set_active_block_number(Security::<T>::active_block_number() + Replace::<T>::replace_period() + 100u32.into());

		VaultRegistry::<T>::_set_secure_collateral_threshold(get_currency_pair::<T>(), UnsignedFixedPoint::<T>::checked_from_rational(1, 100000).unwrap());
		VaultRegistry::<T>::_set_system_collateral_ceiling(get_currency_pair::<T>(), 1_000_000_000u32.into());

		register_vault::<T>(old_vault_id.clone());
		VaultRegistry::<T>::try_increase_to_be_issued_tokens(&old_vault_id, &amount).unwrap();
		VaultRegistry::<T>::issue_tokens(&old_vault_id, &amount).unwrap();
		VaultRegistry::<T>::try_increase_to_be_redeemed_tokens(&old_vault_id, &amount).unwrap();

		register_vault::<T>(new_vault_id.clone());
		VaultRegistry::<T>::try_increase_to_be_issued_tokens(&new_vault_id, &amount).unwrap();

	}: _(RawOrigin::Signed(new_vault_id.account_id), replace_id)

	set_replace_period {
	}: _(RawOrigin::Root, 1u32.into())

	minimum_transfer_amount_update {
		let new_minimum_amount: BalanceOf<T> = 1u32.into();
	}: _(RawOrigin::Root, new_minimum_amount)
}

impl_benchmark_test_suite!(
	Replace,
	crate::mock::ExtBuilder::build_with(Default::default()),
	crate::mock::Test
);
