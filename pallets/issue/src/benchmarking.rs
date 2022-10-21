use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{assert_ok, BoundedVec};
use frame_system::RawOrigin;
use orml_traits::MultiCurrency;
use sp_core::H256;
use sp_runtime::{traits::One, FixedPointNumber};
use sp_std::prelude::*;
use substrate_stellar_sdk::{
	compound_types::{LimitedVarArray, LimitedVarOpaque, UnlimitedVarOpaque},
	network::{Network, PUBLIC_NETWORK, TEST_NETWORK},
	types::{
		NodeId, Preconditions, ScpBallot, ScpStatement, ScpStatementExternalize,
		ScpStatementPledges, Signature, StellarValue, StellarValueExt, TransactionExt,
		TransactionV1Envelope, Value,
	},
	AccountId, Hash, Memo, MuxedAccount, PublicKey, SecretKey, Transaction, XdrCodec,
};

use currency::getters::{get_relay_chain_currency_id as get_collateral_currency_id, *};
use oracle::Pallet as Oracle;
use primitives::{CurrencyId, StellarPublicKeyRaw, VaultCurrencyPair, VaultId};
use security::Pallet as Security;
use stellar_relay::{
	traits::{Organization, Validator},
	types::{OrganizationOf, ValidatorOf},
	Pallet as StellarRelay,
};
use vault_registry::{types::DefaultVaultCurrencyPair, Pallet as VaultRegistry};

// Pallets
use crate::Pallet as Issue;

use super::*;

const STELLAR_PUBLIC_KEY_DUMMY: StellarPublicKeyRaw = [1u8; 32];
const IS_PUBLIC_NETWORK: bool = false;

// These have to match the ones defined in the genesis config of mock.rs
const VALIDATOR_1_SECRET: [u8; 32] = [1u8; 32];
const VALIDATOR_2_SECRET: [u8; 32] = [2u8; 32];
const VALIDATOR_3_SECRET: [u8; 32] = [3u8; 32];

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
		wrapped: get_wrapped_currency_id::<T>(),
	}
}

fn get_vault_id<T: crate::Config>() -> DefaultVaultId<T> {
	VaultId::new(
		account("Vault", 0, 0),
		get_collateral_currency_id::<T>(),
		get_wrapped_currency_id::<T>(),
	)
}

fn register_vault<T: crate::Config>(vault_id: DefaultVaultId<T>) {
	let origin = RawOrigin::Signed(vault_id.account_id.clone());
	assert_ok!(VaultRegistry::<T>::register_public_key(origin.into(), STELLAR_PUBLIC_KEY_DUMMY));
	assert_ok!(VaultRegistry::<T>::_register_vault(vault_id.clone(), 100000000u32.into()));
}

fn create_scp_envelope(
	tx_set_hash: Hash,
	validator_secret_key: &SecretKey,
	network: &Network,
) -> ScpEnvelope {
	let stellar_value: StellarValue = StellarValue {
		tx_set_hash,
		close_time: 0,
		ext: StellarValueExt::StellarValueBasic,
		upgrades: LimitedVarArray::new_empty(),
	};
	let stellar_value_xdr = stellar_value.to_xdr();
	let value: Value = UnlimitedVarOpaque::new(stellar_value_xdr).unwrap();

	let node_id = NodeId::from_encoding(validator_secret_key.get_public().to_encoding()).unwrap();

	let statement: ScpStatement = ScpStatement {
		node_id,
		slot_index: 0,
		pledges: ScpStatementPledges::ScpStExternalize(ScpStatementExternalize {
			commit: ScpBallot { counter: 0, value },
			n_h: 0,
			commit_quorum_set_hash: Hash::default(),
		}),
	};

	let network = network.get_id();
	let envelope_type_scp = [0, 0, 0, 1].to_vec(); // xdr representation
	let body: Vec<u8> = [network.to_vec(), envelope_type_scp, statement.to_xdr()].concat();
	let signature_result = validator_secret_key.create_signature(body);
	let signature: Signature = LimitedVarOpaque::new(signature_result.to_vec()).unwrap();

	let envelope = ScpEnvelope { statement, signature };
	envelope
}

fn get_validators_and_organizations<T: crate::Config>(
) -> (Vec<ValidatorOf<T>>, Vec<OrganizationOf<T>>) {
	// Validators and organizations needed to build valid proof in the benchmark
	let organization: OrganizationOf<T> = Organization {
		id: 1.into(),
		name: BoundedVec::try_from("organization".as_bytes().to_vec()).unwrap(),
		public_network: false,
	};

	let validator_1: ValidatorOf<T> = Validator {
		name: Default::default(),
		public_key: BoundedVec::try_from(
			SecretKey::from_binary(VALIDATOR_1_SECRET).get_public().to_encoding(),
		)
		.unwrap(),
		organization_id: organization.id,
		public_network: false,
	};
	let validator_2: ValidatorOf<T> = Validator {
		name: Default::default(),
		public_key: BoundedVec::try_from(
			SecretKey::from_binary(VALIDATOR_2_SECRET).get_public().to_encoding(),
		)
		.unwrap(),
		organization_id: organization.id,
		public_network: false,
	};
	let validator_3: ValidatorOf<T> = Validator {
		name: Default::default(),
		public_key: BoundedVec::try_from(
			SecretKey::from_binary(VALIDATOR_3_SECRET).get_public().to_encoding(),
		)
		.unwrap(),
		organization_id: organization.id,
		public_network: false,
	};

	(vec![validator_1, validator_2, validator_3], vec![organization])
}

fn build_dummy_proof_for<T: crate::Config>(issue_id: H256) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
	// Build a transaction
	let source_account =
		MuxedAccount::from(AccountId::from(PublicKey::PublicKeyTypeEd25519([0; 32])));
	let operations = LimitedVarArray::new(vec![]).unwrap();
	let transaction = Transaction {
		source_account,
		fee: 0,
		seq_num: 0,
		cond: Preconditions::PrecondNone,
		memo: Memo::MemoHash(Hash::from(issue_id)), // Include the issue id in the memo
		operations,
		ext: TransactionExt::V0,
	};

	let transaction_envelope: TransactionEnvelope =
		TransactionEnvelope::EnvelopeTypeTx(TransactionV1Envelope {
			tx: transaction,
			signatures: LimitedVarArray::new(vec![]).unwrap(),
		});

	// Build a transaction set with the transaction
	let mut txes = UnlimitedVarArray::<TransactionEnvelope>::new_empty();
	// Add the transaction that is to be verified to the transaction set
	txes.push(transaction_envelope.clone()).unwrap();
	let transaction_set = TransactionSet { previous_ledger_hash: Hash::default(), txes };

	let tx_set_hash = stellar_relay::compute_non_generic_tx_set_content_hash(&transaction_set);
	let network: &Network = if IS_PUBLIC_NETWORK { &PUBLIC_NETWORK } else { &TEST_NETWORK };

	// Build the scp messages that externalize the transaction set
	// The scp messages have to be externalized by nodes that build a valid quorum set
	let mut envelopes = UnlimitedVarArray::<ScpEnvelope>::new_empty();
	let validator_secret_keys = vec![VALIDATOR_1_SECRET, VALIDATOR_2_SECRET, VALIDATOR_3_SECRET];
	for validator_secret_key in validator_secret_keys.iter() {
		let secret_key = SecretKey::from_binary(*validator_secret_key);
		let envelope = create_scp_envelope(tx_set_hash.clone(), &secret_key, network);
		envelopes.push(envelope).unwrap();
	}

	let tx_env_xdr_encoded = base64::encode(&transaction_envelope.to_xdr()).as_bytes().to_vec();
	let scp_envs_xdr_encoded = base64::encode(&envelopes.to_xdr()).as_bytes().to_vec();
	let tx_set_xdr_encoded = base64::encode(&transaction_set.to_xdr()).as_bytes().to_vec();
	(tx_env_xdr_encoded, scp_envs_xdr_encoded, tx_set_xdr_encoded)
}

benchmarks! {
	request_issue {
		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();
		let amount = 1000u32.into();
		let asset = vault_id.wrapped_currency();
		let relayer_id: T::AccountId = account("Relayer", 0, 0);

		Oracle::<T>::_set_exchange_rate(get_collateral_currency_id::<T>(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();
		Oracle::<T>::_set_exchange_rate(<T as vault_registry::Config>::GetGriefingCollateralCurrencyId::get(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();

		mint_collateral::<T>(&origin, (1u32 << 31).into());
		mint_collateral::<T>(&vault_id.account_id.clone(), (1u32 << 31).into());
		mint_collateral::<T>(&relayer_id, (1u32 << 31).into());

		VaultRegistry::<T>::_set_secure_collateral_threshold(get_currency_pair::<T>(), <T as currency::Config>::UnsignedFixedPoint::checked_from_rational(1, 100000).unwrap());// 0.001%
		VaultRegistry::<T>::_set_system_collateral_ceiling(get_currency_pair::<T>(), 1_000_000_000u32.into());
		register_vault::<T>(vault_id.clone());

		Security::<T>::set_active_block_number(1u32.into());
	}: _(RawOrigin::Signed(origin), amount, asset, vault_id, IS_PUBLIC_NETWORK)

	execute_issue {
		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();
		let relayer_id: T::AccountId = account("Relayer", 0, 0);

		mint_collateral::<T>(&origin, (1u32 << 31).into());
		mint_collateral::<T>(&vault_id.account_id.clone(), (1u32 << 31).into());
		mint_collateral::<T>(&relayer_id, (1u32 << 31).into());

		let vault_stellar_address = STELLAR_PUBLIC_KEY_DUMMY;
		let value: Amount<T> = Amount::new(2u32.into(), get_wrapped_currency_id::<T>());

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
			public_network: IS_PUBLIC_NETWORK
		};
		Issue::<T>::insert_issue_request(&issue_id, &issue_request);
		Security::<T>::set_active_block_number(1u32.into());

		let (validators, organizations) = get_validators_and_organizations::<T>();
		StellarRelay::<T>::_update_tier_1_validator_set(validators, organizations).unwrap();
		let (tx_env_xdr_encoded, scp_envs_xdr_encoded, tx_set_xdr_encoded) = build_dummy_proof_for::<T>(issue_id);

		VaultRegistry::<T>::_set_system_collateral_ceiling(get_currency_pair::<T>(), 1_000_000_000u32.into());
		VaultRegistry::<T>::_set_secure_collateral_threshold(get_currency_pair::<T>(), <T as currency::Config>::UnsignedFixedPoint::checked_from_rational(1, 100000).unwrap());
		Oracle::<T>::_set_exchange_rate(get_collateral_currency_id::<T>(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();
		register_vault::<T>(vault_id.clone());

		VaultRegistry::<T>::try_increase_to_be_issued_tokens(&vault_id, &value).unwrap();
		let secure_id = Security::<T>::get_secure_id(&vault_id.account_id);
		VaultRegistry::<T>::register_deposit_address(&vault_id, secure_id).unwrap();
	}: _(RawOrigin::Signed(origin), issue_id, tx_env_xdr_encoded, scp_envs_xdr_encoded, tx_set_xdr_encoded)

	cancel_issue {
		let origin: T::AccountId = account("Origin", 0, 0);
		let vault_id = get_vault_id::<T>();

		mint_collateral::<T>(&origin, (1u32 << 31).into());
		mint_collateral::<T>(&vault_id.account_id.clone(), (1u32 << 31).into());

		let vault_stellar_address = STELLAR_PUBLIC_KEY_DUMMY;
		let value = Amount::new(2u32.into(), get_wrapped_currency_id::<T>());

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
			public_network: IS_PUBLIC_NETWORK
		};

		Issue::<T>::insert_issue_request(&issue_id, &issue_request);

		// expire issue request
		Security::<T>::set_active_block_number(issue_request.opentime + Issue::<T>::issue_period() + 100u32.into());

		VaultRegistry::<T>::_set_system_collateral_ceiling(get_currency_pair::<T>(), 1_000_000_000u32.into());
		VaultRegistry::<T>::_set_secure_collateral_threshold(get_currency_pair::<T>(), <T as currency::Config>::UnsignedFixedPoint::checked_from_rational(1, 100000).unwrap());
		Oracle::<T>::_set_exchange_rate(get_collateral_currency_id::<T>(), <T as currency::Config>::UnsignedFixedPoint::one()).unwrap();
		register_vault::<T>(vault_id.clone());

		VaultRegistry::<T>::try_increase_to_be_issued_tokens(&vault_id, &value).unwrap();
		let secure_id = Security::<T>::get_secure_id(&vault_id.account_id);
		VaultRegistry::<T>::register_deposit_address(&vault_id, secure_id).unwrap();

	}: _(RawOrigin::Signed(origin), issue_id)

	set_issue_period {
	}: _(RawOrigin::Root, 1u32.into())

}

impl_benchmark_test_suite!(
	Issue,
	crate::mock::ExtBuilder::build_with(Default::default()),
	crate::mock::Test
);
