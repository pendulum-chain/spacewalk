use frame_support::BoundedVec;

use crate::{
	traits::{FieldLength, Organization, Validator},
	types::{OrganizationIdOf, OrganizationOf, ValidatorOf},
	Config, GenesisConfig,
};

fn create_bounded_vec(input: &str) -> Result<BoundedVec<u8, FieldLength>, ()> {
	let bounded_vec = BoundedVec::try_from(input.as_bytes().to_vec()).map_err(|_| ())?;
	Ok(bounded_vec)
}

pub fn build_default_genesis<T: Config>() -> GenesisConfig<T> {
	// Build the initial tier 1 validator set
	let organizations: Vec<OrganizationOf<T>> = vec![
		Organization {
			id: <T as Config>::OrganizationId::default(),
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

	let validators: Vec<ValidatorOf<T>> = vec![
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

	GenesisConfig { validators, organizations, phantom: Default::default() }
}
