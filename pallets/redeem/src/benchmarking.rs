use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use orml_traits::MultiCurrency;
use sp_core::H256;
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
	Pallet as StellarRelay,
};
use vault_registry::{types::Vault, Pallet as VaultRegistry};

// Pallets
use crate::Pallet as Redeem;

use super::*;

type UnsignedFixedPoint<T> = <T as currency::Config>::UnsignedFixedPoint;

fn collateral<T: crate::Config>(amount: u32) -> Amount<T> {
	Amount::new(amount.into(), get_collateral_currency_id::<T>())
}

fn wrapped<T: crate::Config>(amount: u32) -> Amount<T> {
	Amount::new(amount.into(), get_wrapped_currency_id())
}

fn register_public_key<T: crate::Config>(vault_id: DefaultVaultId<T>) {
	let origin = RawOrigin::Signed(vault_id.account_id);
	assert_ok!(VaultRegistry::<T>::register_public_key(origin.into(), DEFAULT_STELLAR_PUBLIC_KEY));
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

fn mint_wrapped<T: crate::Config>(account_id: &T::AccountId, amount: BalanceOf<T>) {
	let rich_amount = Amount::<T>::new(amount, get_wrapped_currency_id());
	assert_ok!(rich_amount.mint_to(account_id));
}

fn initialize_oracle<T: crate::Config>() {
	let oracle_id: T::AccountId = account("Oracle", 12, 0);

	use primitives::oracle::Key;
	let result = Oracle::<T>::_feed_values(
		oracle_id,
		vec![
			(
				Key::ExchangeRate(get_collateral_currency_id::<T>()),
				UnsignedFixedPoint::<T>::checked_from_rational(1, 1).unwrap(),
			),
			(
				Key::ExchangeRate(get_wrapped_currency_id()),
				UnsignedFixedPoint::<T>::checked_from_rational(1, 1).unwrap(),
			),
		],
	);
	assert_ok!(result);
	Oracle::<T>::begin_block(0u32.into());
}

fn test_request<T: crate::Config>(vault_id: &DefaultVaultId<T>) -> DefaultRedeemRequest<T> {
	RedeemRequest {
		vault: vault_id.clone(),
		opentime: Default::default(),
		period: Default::default(),
		fee: Default::default(),
		transfer_fee: Default::default(),
		amount: Default::default(),
		asset: get_wrapped_currency_id(),
		premium: Default::default(),
		redeemer: account("Redeemer", 0, 0),
		stellar_address: Default::default(),
		status: Default::default(),
	}
}

fn get_vault_id<T: crate::Config>() -> DefaultVaultId<T> {
	VaultId::new(
		account("Vault", 0, 0),
		get_collateral_currency_id::<T>(),
		get_wrapped_currency_id(),
	)
}

benchmarks! {
	request_redeem {
		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();
		let amount = Redeem::<T>::redeem_minimum_transfer_amount() + 1000u32.into();
		let asset = CurrencyId::XCM(0);
		let stellar_address = DEFAULT_STELLAR_PUBLIC_KEY;

		initialize_oracle::<T>();

		register_public_key::<T>(vault_id.clone());

		let vault = Vault {
			issued_tokens: amount,
			id: vault_id.clone(),
			..Vault::new(vault_id.clone())
		};

		VaultRegistry::<T>::insert_vault(
			&vault_id,
			vault
		);

		mint_wrapped::<T>(&origin, amount);

		assert_ok!(Oracle::<T>::_set_exchange_rate(origin.clone(), get_collateral_currency_id::<T>(),
			UnsignedFixedPoint::<T>::one()
		));
	}: _(RawOrigin::Signed(origin), amount, stellar_address, vault_id)

	liquidation_redeem {
		initialize_oracle::<T>();

		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();
		let amount = 1000;

		register_public_key::<T>(vault_id.clone());

		VaultRegistry::<T>::insert_vault(
			&vault_id,
			Vault::new(vault_id.clone())
		);

		mint_wrapped::<T>(&origin, amount.into());

		mint_collateral::<T>(&vault_id.account_id, 100_000u32.into());
		assert_ok!(VaultRegistry::<T>::try_deposit_collateral(&vault_id, &collateral(100_000)));

		assert_ok!(VaultRegistry::<T>::try_increase_to_be_issued_tokens(&vault_id, &wrapped(amount)));
		assert_ok!(VaultRegistry::<T>::issue_tokens(&vault_id, &wrapped(amount)));

		VaultRegistry::<T>::liquidate_vault(&vault_id).unwrap();
		let currency_pair = VaultCurrencyPair {
			collateral: get_collateral_currency_id::<T>(),
			wrapped: get_wrapped_currency_id()
		};
	}: _(RawOrigin::Signed(origin), currency_pair, amount.into())

	execute_redeem {
		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();
		let relayer_id: T::AccountId = account("Relayer", 0, 0);

		initialize_oracle::<T>();

		let origin_stellar_address = DEFAULT_STELLAR_PUBLIC_KEY;

		let redeem_id = H256::zero();
		let mut redeem_request = test_request::<T>(&vault_id);
		redeem_request.stellar_address = origin_stellar_address;
		Redeem::<T>::insert_redeem_request(&redeem_id, &redeem_request);

		register_public_key::<T>(vault_id.clone());

		let vault = Vault {
			id: vault_id.clone(),
			..Vault::new(vault_id.clone())
		};

		VaultRegistry::<T>::insert_vault(
			&vault_id,
			vault
		);

		Security::<T>::set_active_block_number(1u32.into());

		let (validators, organizations) = get_validators_and_organizations::<T>();
		let enactment_block_height = T::BlockNumber::default();
		StellarRelay::<T>::_update_tier_1_validator_set(validators, organizations, enactment_block_height).unwrap();
		let public_network = StellarRelay::<T>::is_public_network();
		let (tx_env_xdr_encoded, scp_envs_xdr_encoded, tx_set_xdr_encoded) = build_dummy_proof_for::<T>(redeem_id, public_network);

		assert_ok!(Oracle::<T>::_set_exchange_rate(origin.clone(), get_collateral_currency_id::<T>(),
			UnsignedFixedPoint::<T>::one()
		));
	}: _(RawOrigin::Signed(vault_id.account_id.clone()), redeem_id, tx_env_xdr_encoded, scp_envs_xdr_encoded, tx_set_xdr_encoded)

	cancel_redeem_reimburse {
		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();

		initialize_oracle::<T>();

		let redeem_id = H256::zero();
		let mut redeem_request = test_request::<T>(&vault_id);
		redeem_request.redeemer = origin.clone();
		redeem_request.opentime = Security::<T>::active_block_number();
		Redeem::<T>::insert_redeem_request(&redeem_id, &redeem_request);

		// expire redeem request
		Security::<T>::set_active_block_number(Security::<T>::active_block_number() + Redeem::<T>::redeem_period() + 100u32.into());

		let vault = Vault {
			id: vault_id.clone(),
			..Vault::new(vault_id.clone())
		};
		VaultRegistry::<T>::insert_vault(
			&vault_id,
			vault
		);
		mint_collateral::<T>(&vault_id.account_id, 1000u32.into());
		assert_ok!(VaultRegistry::<T>::try_deposit_collateral(&vault_id, &collateral(1000)));

		assert_ok!(Oracle::<T>::_set_exchange_rate(origin.clone(), get_collateral_currency_id::<T>(),
			UnsignedFixedPoint::<T>::one()
		));
	}: cancel_redeem(RawOrigin::Signed(origin), redeem_id, true)

	cancel_redeem_retry {
		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();

		initialize_oracle::<T>();

		let redeem_id = H256::zero();
		let mut redeem_request = test_request::<T>(&vault_id);
		redeem_request.redeemer = origin.clone();
		redeem_request.opentime = Security::<T>::active_block_number();
		Redeem::<T>::insert_redeem_request(&redeem_id, &redeem_request);

		// expire redeem request
		Security::<T>::set_active_block_number(Security::<T>::active_block_number() + Redeem::<T>::redeem_period() + 100u32.into());

		let vault = Vault {
			id: vault_id.clone(),
			..Vault::new(vault_id.clone())
		};
		VaultRegistry::<T>::insert_vault(
			&vault_id,
			vault
		);
		mint_collateral::<T>(&vault_id.account_id, 1000u32.into());
		assert_ok!(VaultRegistry::<T>::try_deposit_collateral(&vault_id, &collateral(1000)));

		assert_ok!(Oracle::<T>::_set_exchange_rate(origin.clone(), get_collateral_currency_id::<T>(),
			UnsignedFixedPoint::<T>::one()
		));
	}: cancel_redeem(RawOrigin::Signed(origin), redeem_id, false)

	self_redeem {
		initialize_oracle::<T>();

		let vault_id = get_vault_id::<T>();
		let origin = vault_id.account_id.clone();
		let amount = 1000;

		register_public_key::<T>(vault_id.clone());

		VaultRegistry::<T>::insert_vault(
			&vault_id,
			Vault::new(vault_id.clone())
		);

		mint_wrapped::<T>(&origin, amount.into());

		mint_collateral::<T>(&vault_id.account_id, 100_000u32.into());
		assert_ok!(VaultRegistry::<T>::try_deposit_collateral(&vault_id, &collateral(100_000)));

		assert_ok!(VaultRegistry::<T>::try_increase_to_be_issued_tokens(&vault_id, &wrapped(amount)));
		assert_ok!(VaultRegistry::<T>::issue_tokens(&vault_id, &wrapped(amount)));

		let currency_pair = VaultCurrencyPair {
			collateral: get_collateral_currency_id::<T>(),
			wrapped: get_wrapped_currency_id()
		};
	}: _(RawOrigin::Signed(origin), currency_pair, amount.into())

	set_redeem_period {
	}: _(RawOrigin::Root, 1u32.into())

	mint_tokens_for_reimbursed_redeem {
		initialize_oracle::<T>();

		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();
		let relayer_id: T::AccountId = account("Relayer", 0, 0);

		initialize_oracle::<T>();

		let origin_stellar_address = DEFAULT_STELLAR_PUBLIC_KEY;

		let redeem_id = H256::zero();
		let mut redeem_request = test_request::<T>(&vault_id);
		redeem_request.status = RedeemRequestStatus::Reimbursed(false);
		redeem_request.stellar_address = origin_stellar_address;
		Redeem::<T>::insert_redeem_request(&redeem_id, &redeem_request);

		let vault_id = get_vault_id::<T>();
		let origin = vault_id.account_id.clone();
		let amount = 1000;

		register_public_key::<T>(vault_id.clone());

		VaultRegistry::<T>::insert_vault(
			&vault_id,
			Vault::new(vault_id.clone())
		);

		mint_wrapped::<T>(&origin, amount.into());

		mint_collateral::<T>(&vault_id.account_id, 100_000u32.into());
		assert_ok!(VaultRegistry::<T>::try_deposit_collateral(&vault_id, &collateral(100_000)));

		assert_ok!(VaultRegistry::<T>::try_increase_to_be_issued_tokens(&vault_id, &wrapped(amount)));
		assert_ok!(VaultRegistry::<T>::issue_tokens(&vault_id, &wrapped(amount)));

		let currency_pair = VaultCurrencyPair {
			collateral: get_collateral_currency_id::<T>(),
			wrapped: get_wrapped_currency_id()
		};
	}: _(RawOrigin::Signed(origin), currency_pair, redeem_id)

	rate_limit_update {
		let limit_volume_amount: Option<BalanceOf<T>> = Some(1u32.into());
		let limit_volume_currency_id: T::CurrencyId = get_wrapped_currency_id();
		let interval_length: T::BlockNumber = 1u32.into();
	}: _(RawOrigin::Root, limit_volume_amount, limit_volume_currency_id, interval_length)
}

impl_benchmark_test_suite!(
	Redeem,
	crate::mock::ExtBuilder::build_with(Default::default()),
	crate::mock::Test
);
