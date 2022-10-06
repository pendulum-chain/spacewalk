use std::{convert::TryFrom, str::FromStr};

use frame_support::BoundedVec;
use hex_literal::hex;
use sc_service::ChainType;
use serde_json::{map::Map, Value};
use sp_consensus_aura::ed25519::AuthorityId as AuraId;
use sp_core::{crypto::UncheckedInto, ed25519, sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::traits::{IdentifyAccount, Verify};

use spacewalk_runtime::{
	AccountId, AuraConfig, BalancesConfig, CurrencyId, FieldLength, GenesisConfig, GrandpaConfig,
	Organization, OrganizationOf, Runtime, Signature, StellarRelayConfig, SudoConfig, SystemConfig,
	TokensConfig, Validator, ValidatorOf, WASM_BINARY,
};

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

fn create_bounded_vec(input: &str) -> Result<BoundedVec<T, FieldLength>, ()> {
	let bounded_vec = BoundedVec::try_from(input.as_bytes().to_vec()).map_err(|_| ())?;
	Ok(bounded_vec)
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

	// Build the initial tier 1 validator set
	let organizations: Vec<OrganizationOf<Runtime>> = vec![
		Organization {
			id: 0,
			name: create_bounded_vec("satoshipay").expect("Couldn't create vec"),
		},
		Organization { id: 1, name: create_bounded_vec("sdf").expect("Couldn't create vec") },
		Organization { id: 2, name: create_bounded_vec("wirex").expect("Couldn't create vec") },
		Organization { id: 3, name: create_bounded_vec("coinqvest").expect("Couldn't create vec") },
		Organization {
			id: 4,
			name: create_bounded_vec("blockdaemon").expect("Couldn't create vec"),
		},
		Organization { id: 5, name: create_bounded_vec("lobstr").expect("Couldn't create vec") },
		Organization {
			id: 6,
			name: create_bounded_vec("public_node").expect("Couldn't create vec"),
		},
	];

	let validators: Vec<ValidatorOf<Runtime>> = vec![
		// Satoshipay validators
		Validator {
			name: create_bounded_vec("$satoshipay-us").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GAK6Z5UVGUVSEK6PEOCAYJISTT5EJBB34PN3NOLEQG2SUKXRVV2F6HZY",
			)
			.expect("Couldn't create vec"),
			organization_id: 0,
		},
		Validator {
			name: create_bounded_vec("$satoshipay-de").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GC5SXLNAM3C4NMGK2PXK4R34B5GNZ47FYQ24ZIBFDFOCU6D4KBN4POAE",
			)
			.expect("Couldn't create vec"),
			organization_id: 0,
		},
		Validator {
			name: create_bounded_vec("$satoshipay-sg").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GBJQUIXUO4XSNPAUT6ODLZUJRV2NPXYASKUBY4G5MYP3M47PCVI55MNT",
			)
			.expect("Couldn't create vec"),
			organization_id: 0,
		},
		// SDF validators
		Validator {
			name: create_bounded_vec("$sdf1").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GCGB2S2KGYARPVIA37HYZXVRM2YZUEXA6S33ZU5BUDC6THSB62LZSTYH",
			)
			.expect("Couldn't create vec"),
			organization_id: 1,
		},
		Validator {
			name: create_bounded_vec("$sdf2").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GCM6QMP3DLRPTAZW2UZPCPX2LF3SXWXKPMP3GKFZBDSF3QZGV2G5QSTK",
			)
			.expect("Couldn't create vec"),
			organization_id: 1,
		},
		Validator {
			name: create_bounded_vec("$sdf3").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GABMKJM6I25XI4K7U6XWMULOUQIQ27BCTMLS6BYYSOWKTBUXVRJSXHYQ",
			)
			.expect("Couldn't create vec"),
			organization_id: 1,
		},
		// Wirex validators
		Validator {
			name: create_bounded_vec("$wirex-sg").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GAB3GZIE6XAYWXGZUDM4GMFFLJBFMLE2JDPUCWUZXMOMT3NHXDHEWXAS",
			)
			.expect("Couldn't create vec"),
			organization_id: 2,
		},
		Validator {
			name: create_bounded_vec("$wirex-us").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GDXUKFGG76WJC7ACEH3JUPLKM5N5S76QSMNDBONREUXPCZYVPOLFWXUS",
			)
			.expect("Couldn't create vec"),
			organization_id: 2,
		},
		Validator {
			name: create_bounded_vec("$wirex-uk").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GBBQQT3EIUSXRJC6TGUCGVA3FVPXVZLGG3OJYACWBEWYBHU46WJLWXEU",
			)
			.expect("Couldn't create vec"),
			organization_id: 2,
		},
		// Coinqvest validators
		Validator {
			name: create_bounded_vec("$coinqvest-germany").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GD6SZQV3WEJUH352NTVLKEV2JM2RH266VPEM7EH5QLLI7ZZAALMLNUVN",
			)
			.expect("Couldn't create vec"),
			organization_id: 3,
		},
		Validator {
			name: create_bounded_vec("$coinqvest-finland").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GADLA6BJK6VK33EM2IDQM37L5KGVCY5MSHSHVJA4SCNGNUIEOTCR6J5T",
			)
			.expect("Couldn't create vec"),
			organization_id: 3,
		},
		Validator {
			name: create_bounded_vec("$coinqvest-hongkong").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GAZ437J46SCFPZEDLVGDMKZPLFO77XJ4QVAURSJVRZK2T5S7XUFHXI2Z",
			)
			.expect("Couldn't create vec"),
			organization_id: 3,
		},
		// Blockdaemon validators
		Validator {
			name: create_bounded_vec("$blockdaemon1").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GAAV2GCVFLNN522ORUYFV33E76VPC22E72S75AQ6MBR5V45Z5DWVPWEU",
			)
			.expect("Couldn't create vec"),
			organization_id: 4,
		},
		Validator {
			name: create_bounded_vec("$blockdaemon2").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GAVXB7SBJRYHSG6KSQHY74N7JAFRL4PFVZCNWW2ARI6ZEKNBJSMSKW7C",
			)
			.expect("Couldn't create vec"),
			organization_id: 4,
		},
		Validator {
			name: create_bounded_vec("$blockdaemon3").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GAYXZ4PZ7P6QOX7EBHPIZXNWY4KCOBYWJCA4WKWRKC7XIUS3UJPT6EZ4",
			)
			.expect("Couldn't create vec"),
			organization_id: 4,
		},
		// LOBSTR validators
		Validator {
			name: create_bounded_vec("$lobstr1").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GCFONE23AB7Y6C5YZOMKUKGETPIAJA4QOYLS5VNS4JHBGKRZCPYHDLW7",
			)
			.expect("Couldn't create vec"),
			organization_id: 5,
		},
		Validator {
			name: create_bounded_vec("$lobstr2").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GDXQB3OMMQ6MGG43PWFBZWBFKBBDUZIVSUDAZZTRAWQZKES2CDSE5HKJ",
			)
			.expect("Couldn't create vec"),
			organization_id: 5,
		},
		Validator {
			name: create_bounded_vec("$lobstr3").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GD5QWEVV4GZZTQP46BRXV5CUMMMLP4JTGFD7FWYJJWRL54CELY6JGQ63",
			)
			.expect("Couldn't create vec"),
			organization_id: 5,
		},
		Validator {
			name: create_bounded_vec("$lobstr4").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GA7TEPCBDQKI7JQLQ34ZURRMK44DVYCIGVXQQWNSWAEQR6KB4FMCBT7J",
			)
			.expect("Couldn't create vec"),
			organization_id: 5,
		},
		Validator {
			name: create_bounded_vec("$lobstr5").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GA5STBMV6QDXFDGD62MEHLLHZTPDI77U3PFOD2SELU5RJDHQWBR5NNK7",
			)
			.expect("Couldn't create vec"),
			organization_id: 5,
		},
		// Public Node validators
		Validator {
			name: create_bounded_vec("$hercules").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GBLJNN3AVZZPG2FYAYTYQKECNWTQYYUUY2KVFN2OUKZKBULXIXBZ4FCT",
			)
			.expect("Couldn't create vec"),
			organization_id: 6,
		},
		Validator {
			name: create_bounded_vec("$boötes").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GCVJ4Z6TI6Z2SOGENSPXDQ2U4RKH3CNQKYUHNSSPYFPNWTLGS6EBH7I2",
			)
			.expect("Couldn't create vec"),
			organization_id: 6,
		},
		Validator {
			name: create_bounded_vec("$lyra").expect("Couldn't create vec"),
			public_key: create_bounded_vec(
				"GCIXVKNFPKWVMKJKVK2V4NK7D4TC6W3BUMXSIJ365QUAXWBRPPJXIR2Z",
			)
			.expect("Couldn't create vec"),
			organization_id: 6,
		},
	];

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
		stellar_relay: StellarRelayConfig {
			validators,
			organizations,
			phantom: Default::default(),
		},
	}
}
