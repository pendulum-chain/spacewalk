use spacewalk_runtime_testnet::{AssetId, DiaOracleModuleConfig};
use std::convert::TryFrom;

use frame_support::BoundedVec;
use sc_service::ChainType;
use sp_arithmetic::{FixedPointNumber, FixedU128};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

use primitives::{oracle::Key, CurrencyId, VaultCurrencyPair};
use serde_json::{map::Map, Value};
use spacewalk_runtime_testnet::{
	AccountId, AuraConfig, BalancesConfig, FeeConfig, FieldLength, GrandpaConfig, RuntimeGenesisConfig,
	IssueConfig, NominationConfig, OracleConfig, Organization, RedeemConfig, ReplaceConfig,
	SecurityConfig, Signature, StatusCode, StellarRelayConfig, SudoConfig, SystemConfig,
	TokensConfig, Validator, VaultRegistryConfig, DAYS,
};

// The URL for the telemetry server.
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

pub type ChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig>;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

// For mainnet USDC issued by centre.io
const WRAPPED_CURRENCY_ID_STELLAR_MAINNET: CurrencyId = CurrencyId::AlphaNum4(
	*b"USDC",
	[
		59, 153, 17, 56, 14, 254, 152, 139, 160, 168, 144, 14, 177, 207, 228, 79, 54, 111, 125,
		190, 148, 107, 237, 7, 114, 64, 247, 246, 36, 223, 21, 197,
	],
);

// For Testnet USDC issued by
const WRAPPED_CURRENCY_ID_STELLAR_TESTNET: CurrencyId = CurrencyId::AlphaNum4(
	*b"USDC",
	[
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
);

const MXN_CURRENCY_ID: CurrencyId = CurrencyId::AlphaNum4(
	*b"MXN\0",
	[
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
);

/// Generate an Aura authority key.
pub fn authority_keys_from_seed(s: &str) -> (AuraId, GrandpaId) {
	(get_from_seed::<AuraId>(s), get_from_seed::<GrandpaId>(s))
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

fn get_properties() -> Map<String, Value> {
	let mut properties = Map::new();
	properties.insert("ss58Format".into(), spacewalk_runtime_testnet::SS58Prefix::get().into());
	properties
}

pub fn mainnet_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"spacewalk",
		"dev_mainnet",
		ChainType::Development,
		move || {
			genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![authority_keys_from_seed("Alice")],
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<sr25519::Public>("Dave"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
					get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
				],
				vec![get_account_id_from_seed::<sr25519::Public>("Bob")],
				false,
				true,
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Some(get_properties()),
		None,
	)
}

pub fn testnet_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"spacewalk",
		"dev_testnet",
		ChainType::Development,
		move || {
			genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![authority_keys_from_seed("Alice")],
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<sr25519::Public>("Dave"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
					get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
				],
				vec![get_account_id_from_seed::<sr25519::Public>("Bob")],
				false,
				false,
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Some(get_properties()),
		None,
	)
}

fn default_pair(currency_id: CurrencyId, is_public_network: bool) -> VaultCurrencyPair<CurrencyId> {
	let wrapped = if is_public_network {
		WRAPPED_CURRENCY_ID_STELLAR_MAINNET
	} else {
		WRAPPED_CURRENCY_ID_STELLAR_TESTNET
	};
	VaultCurrencyPair { collateral: currency_id, wrapped }
}

// Used to create bounded vecs for genesis config
// Does not return a result but panics because the genesis config is hardcoded
fn create_bounded_vec(input: &str) -> BoundedVec<u8, FieldLength> {
	let bounded_vec =
		BoundedVec::try_from(input.as_bytes().to_vec()).expect("Failed to create bounded vec");

	bounded_vec
}

fn genesis(
	root_key: AccountId,
	initial_authorities: Vec<(AuraId, GrandpaId)>,
	endowed_accounts: Vec<AccountId>,
	authorized_oracles: Vec<AccountId>,
	start_shutdown: bool,
	is_public_network: bool,
) -> RuntimeGenesisConfig {
	let default_wrapped_currency = if is_public_network {
		WRAPPED_CURRENCY_ID_STELLAR_MAINNET
	} else {
		WRAPPED_CURRENCY_ID_STELLAR_TESTNET
	};

	// It's very important that we use the correct wasm binary
	let wasm_binary = if is_public_network {
		spacewalk_runtime_mainnet::WASM_BINARY
	} else {
		spacewalk_runtime_testnet::WASM_BINARY
	};

	RuntimeGenesisConfig {
		system: SystemConfig {
			code: wasm_binary.expect("WASM binary was not build, please build it!").to_vec(),
			..Default::default()
		},
		aura: AuraConfig {
			authorities: initial_authorities.iter().map(|x| (x.0.clone())).collect(),
		},
		grandpa: GrandpaConfig {
			authorities: initial_authorities.iter().map(|x| (x.1.clone(), 1)).collect(),
			..Default::default()
		},
		sudo: SudoConfig {
			// Assign network admin rights.
			key: Some(root_key),
		},
		balances: BalancesConfig {
			// Configure endowed accounts with initial balance of 1 << 60.
			balances: endowed_accounts.iter().cloned().map(|k| (k, 1 << 60)).collect(),
		},
		tokens: TokensConfig {
			// Configure the initial token supply for the native currency and USDC asset
			balances: endowed_accounts
				.iter()
				.flat_map(|k| {
					vec![
						(k.clone(), CurrencyId::XCM(0), 1 << 60),
						(k.clone(), CurrencyId::XCM(1), 1 << 60),
						(k.clone(), CurrencyId::Native, 1 << 60),
					]
				})
				.collect(),
		},
		issue: IssueConfig {
			issue_period: DAYS,
			issue_minimum_transfer_amount: 100_00000, // This is 100 Stellar stroops
			limit_volume_amount: None,
			limit_volume_currency_id: CurrencyId::XCM(0),
			current_volume_amount: 0u128,
			interval_length: (60u32 * 60 * 24),
			last_interval_index: 0u32,
		},
		redeem: RedeemConfig {
			redeem_period: DAYS,
			redeem_minimum_transfer_amount: 100_00000, // This is 100 Stellar stroops
			limit_volume_amount: None,
			limit_volume_currency_id: CurrencyId::XCM(0),
			current_volume_amount: 0u128,
			interval_length: (60u32 * 60 * 24),
			last_interval_index: 0u32,
		},
		replace: ReplaceConfig {
			replace_period: DAYS,
			replace_minimum_transfer_amount: 100_00000, // This is 100 Stellar stroops
		},
		security: SecurityConfig {
			initial_status: if start_shutdown { StatusCode::Shutdown } else { StatusCode::Error },
			..Default::default()
		},
		stellar_relay: if !is_public_network {
			create_stellar_testnet_config()
		} else {
			StellarRelayConfig::default()
		},
		oracle: OracleConfig {
			max_delay: u32::MAX,
			oracle_keys: vec![
				// Changing these items means that the integration tests also have to change
				// because the integration tests insert dummy values for these into the oracle
				Key::ExchangeRate(CurrencyId::XCM(0)),
				Key::ExchangeRate(default_wrapped_currency),
				Key::ExchangeRate(MXN_CURRENCY_ID),
			],
			..Default::default()
		},
		vault_registry: VaultRegistryConfig {
			minimum_collateral_vault: vec![
				(CurrencyId::XCM(0), 0),
				(CurrencyId::XCM(1), 0),
				(CurrencyId::StellarNative, 0),
			],
			punishment_delay: DAYS,
			secure_collateral_threshold: vec![
				(
					default_pair(CurrencyId::XCM(0), is_public_network),
					FixedU128::checked_from_rational(160, 100).unwrap(),
				),
				(
					default_pair(CurrencyId::XCM(1), is_public_network),
					FixedU128::checked_from_rational(160, 100).unwrap(),
				),
				(
					VaultCurrencyPair { collateral: CurrencyId::XCM(0), wrapped: MXN_CURRENCY_ID },
					FixedU128::checked_from_rational(160, 100).unwrap(),
				),
				(
					VaultCurrencyPair {
						collateral: CurrencyId::XCM(0),
						wrapped: CurrencyId::StellarNative,
					},
					FixedU128::checked_from_rational(160, 100).unwrap(),
				),
			],
			/* 150% */
			premium_redeem_threshold: vec![
				(
					default_pair(CurrencyId::XCM(0), is_public_network),
					FixedU128::checked_from_rational(140, 100).unwrap(),
				),
				(
					default_pair(CurrencyId::XCM(1), is_public_network),
					FixedU128::checked_from_rational(140, 100).unwrap(),
				),
				(
					VaultCurrencyPair {
						collateral: CurrencyId::StellarNative,
						wrapped: MXN_CURRENCY_ID,
					},
					FixedU128::checked_from_rational(140, 100).unwrap(),
				),
				(
					VaultCurrencyPair { collateral: CurrencyId::XCM(0), wrapped: MXN_CURRENCY_ID },
					FixedU128::checked_from_rational(140, 100).unwrap(),
				),
				(
					VaultCurrencyPair {
						collateral: CurrencyId::XCM(0),
						wrapped: CurrencyId::StellarNative,
					},
					FixedU128::checked_from_rational(140, 100).unwrap(),
				),
			],
			/* 135% */
			liquidation_collateral_threshold: vec![
				(
					default_pair(CurrencyId::XCM(0), is_public_network),
					FixedU128::checked_from_rational(120, 100).unwrap(),
				),
				(
					default_pair(CurrencyId::XCM(1), is_public_network),
					FixedU128::checked_from_rational(120, 100).unwrap(),
				),
				(
					VaultCurrencyPair { collateral: CurrencyId::XCM(0), wrapped: MXN_CURRENCY_ID },
					FixedU128::checked_from_rational(160, 100).unwrap(),
				),
				(
					VaultCurrencyPair {
						collateral: CurrencyId::XCM(0),
						wrapped: CurrencyId::StellarNative,
					},
					FixedU128::checked_from_rational(160, 100).unwrap(),
				),
			],
			/* 110% */
			system_collateral_ceiling: vec![
				(default_pair(CurrencyId::XCM(0), is_public_network), 60_000 * 10u128.pow(12)),
				(default_pair(CurrencyId::XCM(1), is_public_network), 60_000 * 10u128.pow(12)),
				(
					VaultCurrencyPair { collateral: CurrencyId::XCM(0), wrapped: MXN_CURRENCY_ID },
					60_000 * 10u128.pow(12),
				),
				(
					VaultCurrencyPair {
						collateral: CurrencyId::XCM(0),
						wrapped: CurrencyId::StellarNative,
					},
					60_000 * 10u128.pow(12),
				),
			],
		},
		fee: FeeConfig {
			issue_fee: FixedU128::checked_from_rational(1, 1000).unwrap(), // 0.1%
			issue_griefing_collateral: FixedU128::checked_from_rational(5, 1000).unwrap(), // 0.5%
			redeem_fee: FixedU128::checked_from_rational(1, 1000).unwrap(), // 0.1%
			premium_redeem_fee: FixedU128::checked_from_rational(5, 100).unwrap(), // 5%
			punishment_fee: FixedU128::checked_from_rational(1, 10).unwrap(), // 10%
			replace_griefing_collateral: FixedU128::checked_from_rational(1, 10).unwrap(), // 10%
		},
		nomination: NominationConfig { is_nomination_enabled: false, ..Default::default() },
		dia_oracle_module: DiaOracleModuleConfig {
			authorized_accounts: authorized_oracles,
			supported_currencies: vec![
				AssetId::new(b"Kusama".to_vec(), b"KSM".to_vec()),
				AssetId::new(b"Polkadot".to_vec(), b"DOT".to_vec()),
				// The order of currencies in the FIAT symbols matter and USD should always be the
				// target currency ie the second one in the pair
				AssetId::new(b"FIAT".to_vec(), b"USD-USD".to_vec()),
				AssetId::new(b"FIAT".to_vec(), b"MXN-USD".to_vec()),
				AssetId::new(b"Stellar".to_vec(), b"XLM".to_vec()),
			],
			batching_api: b"https://dia-00.pendulumchain.tech:8070/currencies".to_vec(),
			coin_infos_map: vec![],
		},
	}
}

fn create_stellar_testnet_config() -> StellarRelayConfig {
	let old_validators = Vec::new();
	let old_organizations = Vec::new();

	// Testnet organization
	let organization_testnet_sdf = Organization { name: create_bounded_vec("sdftest"), id: 1u128 };
	// Testnet validators
	let validators = vec![
		Validator {
			name: create_bounded_vec("$sdftest1"),
			public_key: create_bounded_vec(
				"GDKXE2OZMJIPOSLNA6N6F2BVCI3O777I2OOC4BV7VOYUEHYX7RTRYA7Y",
			),
			organization_id: organization_testnet_sdf.id,
		},
		Validator {
			name: create_bounded_vec("$sdftest2"),
			public_key: create_bounded_vec(
				"GCUCJTIYXSOXKBSNFGNFWW5MUQ54HKRPGJUTQFJ5RQXZXNOLNXYDHRAP",
			),
			organization_id: organization_testnet_sdf.id,
		},
		Validator {
			name: create_bounded_vec("$sdftest3"),
			public_key: create_bounded_vec(
				"GC2V2EFSXN6SQTWVYA5EPJPBWWIMSD2XQNKUOHGEKB535AQE2I6IXV2Z",
			),
			organization_id: organization_testnet_sdf.id,
		},
	];
	let organizations = vec![organization_testnet_sdf];

	StellarRelayConfig {
		old_validators,
		old_organizations,
		validators,
		organizations,
		enactment_block_height: 0,
		phantom: Default::default(),
	}
}
