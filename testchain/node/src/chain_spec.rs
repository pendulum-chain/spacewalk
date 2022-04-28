use hex_literal::hex;
use sc_service::ChainType;
use sp_consensus_aura::ed25519::AuthorityId as AuraId;
use sp_core::{crypto::UncheckedInto, ed25519, sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::traits::{IdentifyAccount, Verify};
use spacewalk_rpc::jsonrpc_core::serde_json::{map::Map, Value};
use spacewalk_runtime::{
	AccountId, AuraConfig, BalancesConfig, CurrencyId, GenesisConfig, GrandpaConfig, Signature,
	SudoConfig, SystemConfig, TokensConfig, WASM_BINARY,
};
use std::{convert::TryFrom, str::FromStr};

// The URL for the telemetry server.
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Generate an Aura authority key.
pub fn authority_keys_from_seed(s: &str) -> (AuraId, GrandpaId) {
	(get_from_seed::<AuraId>(s), get_from_seed::<GrandpaId>(s))
}

fn get_account_id_from_string(account_id: &str) -> AccountId {
	AccountId::from_str(account_id).expect("account id is not valid")
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
	properties.insert("ss58Format".into(), spacewalk_runtime::SS58Prefix::get().into());
	properties
}

pub fn local_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"spacewalk",
		"local_testnet",
		ChainType::Local,
		move || {
			testnet_genesis(
				get_account_id_from_seed::<ed25519::Public>("Alice"),
				vec![],
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<ed25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<ed25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<ed25519::Public>("Charlie"),
					get_account_id_from_seed::<ed25519::Public>("Dave"),
					get_account_id_from_seed::<ed25519::Public>("Eve"),
					get_account_id_from_seed::<ed25519::Public>("Ferdie"),
					get_account_id_from_seed::<ed25519::Public>("Alice//stash"),
					get_account_id_from_seed::<ed25519::Public>("Bob//stash"),
					get_account_id_from_seed::<ed25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<ed25519::Public>("Dave//stash"),
					get_account_id_from_seed::<ed25519::Public>("Eve//stash"),
					get_account_id_from_seed::<ed25519::Public>("Ferdie//stash"),
				],
			)
		},
		vec![],
		None,
		None,
		None,
		Some(get_properties()),
		None,
	)
}

pub fn beta_testnet_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"spacewalk",
		"beta_testnet",
		ChainType::Live,
		move || {
			testnet_genesis(
				get_account_id_from_string("5HeVGqvfpabwFqzV1DhiQmjaLQiFcTSmq2sH6f7atsXkgvtt"),
				vec![
					(
						// 5DJ3wbdicFSFFudXndYBuvZKjucTsyxtJX5WPzQM8HysSkFY
						hex!["366a092a27b4b28199a588b0155a2c9f3f0513d92481de4ee2138273926fa91c"]
							.unchecked_into(),
						hex!["dce82040dc0a90843897aee1cc1a96c205fe7c1165b8f46635c2547ed15a3013"]
							.unchecked_into(),
					),
					(
						// 5HW7ApFamN6ovtDkFyj67tRLRhp8B2kVNjureRUWWYhkTg9j
						hex!["f08cc7cf45f88e6dbe312a63f6ce639061834b4208415b235f77a67b51435f63"]
							.unchecked_into(),
						hex!["5b4651cf045ddf55f0df7bfbb9bb4c45bbeb3c536c6ce4a98275781b8f0f0754"]
							.unchecked_into(),
					),
					(
						// 5FNbq8zGPZtinsfgyD4w2G3BMh75H3r2Qg3uKudTZkJtRru6
						hex!["925ad4bdf35945bea91baeb5419a7ffa07002c6a85ba334adfa7cb5b05623c1b"]
							.unchecked_into(),
						hex!["8de3db7b51864804d2dd5c5905d571aa34d7161537d5a0045755b72d1ac2062e"]
							.unchecked_into(),
					),
				],
				vec![
					// root key
					get_account_id_from_string("5HeVGqvfpabwFqzV1DhiQmjaLQiFcTSmq2sH6f7atsXkgvtt"),
					// faucet
					get_account_id_from_string("5FHy3cvyToZ4ConPXhi43rycAcGYw2R2a8cCjfVMfyuS1Ywg"),
					// vaults
					get_account_id_from_string("5F7Q9FqnGwJmjLtsFGymHZXPEx2dWRVE7NW4Sw2jzEhUB5WQ"),
					get_account_id_from_string("5CJncqjWDkYv4P6nccZHGh8JVoEBXvharMqVpkpJedoYNu4A"),
					get_account_id_from_string("5GpnEWKTWv7xiQtDFi9Rku7DrvgHj4oqMDev4qBQhfwQE8nx"),
					get_account_id_from_string("5DttG269R1NTBDWcghYxa9NmV2wHxXpTe4U8pu4jK3LCE9zi"),
					// relayers
					get_account_id_from_string("5DNzULM1UJXDM7NUgDL4i8Hrhe9e3vZkB3ByM1eEXMGAs4Bv"),
					get_account_id_from_string("5GEXRnnv8Qz9rEwMs4TfvHme48HQvVTEDHJECCvKPzFB4pFZ"),
					// oracles
					get_account_id_from_string("5H8zjSWfzMn86d1meeNrZJDj3QZSvRjKxpTfuVaZ46QJZ4qs"),
					get_account_id_from_string("5FPBT2BVVaLveuvznZ9A1TUtDcbxK5yvvGcMTJxgFmhcWGwj"),
				],
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

pub fn development_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"spacewalk",
		"dev_testnet",
		ChainType::Development,
		move || {
			testnet_genesis(
				get_account_id_from_seed::<ed25519::Public>("Alice"),
				vec![authority_keys_from_seed("Alice")],
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<ed25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<ed25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<ed25519::Public>("Charlie"),
					get_account_id_from_seed::<ed25519::Public>("Dave"),
					get_account_id_from_seed::<ed25519::Public>("Eve"),
					get_account_id_from_seed::<ed25519::Public>("Ferdie"),
					get_account_id_from_seed::<ed25519::Public>("Alice//stash"),
					get_account_id_from_seed::<ed25519::Public>("Bob//stash"),
					get_account_id_from_seed::<ed25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<ed25519::Public>("Dave//stash"),
					get_account_id_from_seed::<ed25519::Public>("Eve//stash"),
					get_account_id_from_seed::<ed25519::Public>("Ferdie//stash"),
				],
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

fn testnet_genesis(
	root_key: AccountId,
	initial_authorities: Vec<(AuraId, GrandpaId)>,
	endowed_accounts: Vec<AccountId>,
) -> GenesisConfig {
	let stellar_usdc_asset: CurrencyId = CurrencyId::try_from((
		"USDC",
		substrate_stellar_sdk::PublicKey::from_encoding(
			"GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC",
		)
		.unwrap()
		.as_binary()
		.clone(),
	))
	.unwrap();

	GenesisConfig {
		system: SystemConfig {
			code: WASM_BINARY.expect("WASM binary was not build, please build it!").to_vec(),
		},
		aura: AuraConfig {
			authorities: initial_authorities.iter().map(|x| (x.0.clone())).collect(),
		},
		grandpa: GrandpaConfig {
			authorities: initial_authorities.iter().map(|x| (x.1.clone(), 1)).collect(),
		},
		sudo: SudoConfig {
			// Assign network admin rights.
			key: Some(root_key.clone()),
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
						(k.clone(), CurrencyId::Native, 1 << 60),
						(k.clone(), stellar_usdc_asset, 1 << 60),
					]
				})
				.collect(),
		},
	}
}
