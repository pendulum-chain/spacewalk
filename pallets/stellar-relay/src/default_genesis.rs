use frame_support::BoundedVec;
use sp_std::{vec, vec::Vec};

use crate::{
	traits::{FieldLength, Validator},
	types::{OrganizationOf, ValidatorOf},
	Config, GenesisConfig,
};

fn create_bounded_vec(input: &str) -> Result<BoundedVec<u8, FieldLength>, ()> {
	let bounded_vec = BoundedVec::try_from(input.as_bytes().to_vec())?;
	Ok(bounded_vec)
}

pub fn build_default_genesis<T: Config>() -> Result<GenesisConfig<T>, ()> {
	// Build the initial tier 1 validator set
	let organization_satoshipay =
		OrganizationOf::<T> { name: create_bounded_vec("SatoshiPay")?, id: 0.into() };
	let organization_sdf = OrganizationOf::<T> {
		name: create_bounded_vec("Stellar Development Foundation")?,
		id: 1.into(),
	};
	let organization_wirex =
		OrganizationOf::<T> { name: create_bounded_vec("Wirex")?, id: 2.into() };
	let organization_coinqvest =
		OrganizationOf::<T> { name: create_bounded_vec("Coinqvest")?, id: 3.into() };
	let organization_blockdaemon =
		OrganizationOf::<T> { name: create_bounded_vec("Blockdaemon")?, id: 4.into() };
	let organization_lobstr =
		OrganizationOf::<T> { name: create_bounded_vec("LOBSTR")?, id: 5.into() };
	let organization_public_node =
		OrganizationOf::<T> { name: create_bounded_vec("Public Node")?, id: 6.into() };

	let validators: Vec<ValidatorOf<T>> = vec![
		// Satoshipay validators
		Validator {
			name: create_bounded_vec("$satoshipay-us")?,
			public_key: create_bounded_vec(
				"GAK6Z5UVGUVSEK6PEOCAYJISTT5EJBB34PN3NOLEQG2SUKXRVV2F6HZY",
			)?,
			organization_id: organization_satoshipay.id,
		},
		Validator {
			name: create_bounded_vec("$satoshipay-de")?,
			public_key: create_bounded_vec(
				"GC5SXLNAM3C4NMGK2PXK4R34B5GNZ47FYQ24ZIBFDFOCU6D4KBN4POAE",
			)?,
			organization_id: organization_satoshipay.id,
		},
		Validator {
			name: create_bounded_vec("$satoshipay-sg")?,
			public_key: create_bounded_vec(
				"GBJQUIXUO4XSNPAUT6ODLZUJRV2NPXYASKUBY4G5MYP3M47PCVI55MNT",
			)?,
			organization_id: organization_satoshipay.id,
		},
		// SDF validators
		Validator {
			name: create_bounded_vec("$sdf1")?,
			public_key: create_bounded_vec(
				"GCGB2S2KGYARPVIA37HYZXVRM2YZUEXA6S33ZU5BUDC6THSB62LZSTYH",
			)?,
			organization_id: organization_sdf.id,
		},
		Validator {
			name: create_bounded_vec("$sdf2")?,
			public_key: create_bounded_vec(
				"GCM6QMP3DLRPTAZW2UZPCPX2LF3SXWXKPMP3GKFZBDSF3QZGV2G5QSTK",
			)?,
			organization_id: organization_sdf.id,
		},
		Validator {
			name: create_bounded_vec("$sdf3")?,
			public_key: create_bounded_vec(
				"GABMKJM6I25XI4K7U6XWMULOUQIQ27BCTMLS6BYYSOWKTBUXVRJSXHYQ",
			)?,
			organization_id: organization_sdf.id,
		},
		// Wirex validators
		Validator {
			name: create_bounded_vec("$wirex-sg")?,
			public_key: create_bounded_vec(
				"GAB3GZIE6XAYWXGZUDM4GMFFLJBFMLE2JDPUCWUZXMOMT3NHXDHEWXAS",
			)?,
			organization_id: organization_wirex.id,
		},
		Validator {
			name: create_bounded_vec("$wirex-us")?,
			public_key: create_bounded_vec(
				"GDXUKFGG76WJC7ACEH3JUPLKM5N5S76QSMNDBONREUXPCZYVPOLFWXUS",
			)?,
			organization_id: organization_wirex.id,
		},
		Validator {
			name: create_bounded_vec("$wirex-uk")?,
			public_key: create_bounded_vec(
				"GBBQQT3EIUSXRJC6TGUCGVA3FVPXVZLGG3OJYACWBEWYBHU46WJLWXEU",
			)?,
			organization_id: organization_wirex.id,
		},
		// Coinqvest validators
		Validator {
			name: create_bounded_vec("$coinqvest-germany")?,
			public_key: create_bounded_vec(
				"GD6SZQV3WEJUH352NTVLKEV2JM2RH266VPEM7EH5QLLI7ZZAALMLNUVN",
			)?,
			organization_id: organization_coinqvest.id,
		},
		Validator {
			name: create_bounded_vec("$coinqvest-finland")?,
			public_key: create_bounded_vec(
				"GADLA6BJK6VK33EM2IDQM37L5KGVCY5MSHSHVJA4SCNGNUIEOTCR6J5T",
			)?,
			organization_id: organization_coinqvest.id,
		},
		Validator {
			name: create_bounded_vec("$coinqvest-hongkong")?,
			public_key: create_bounded_vec(
				"GAZ437J46SCFPZEDLVGDMKZPLFO77XJ4QVAURSJVRZK2T5S7XUFHXI2Z",
			)?,
			organization_id: organization_coinqvest.id,
		},
		// Blockdaemon validators
		Validator {
			name: create_bounded_vec("$blockdaemon1")?,
			public_key: create_bounded_vec(
				"GAAV2GCVFLNN522ORUYFV33E76VPC22E72S75AQ6MBR5V45Z5DWVPWEU",
			)?,
			organization_id: organization_blockdaemon.id,
		},
		Validator {
			name: create_bounded_vec("$blockdaemon2")?,
			public_key: create_bounded_vec(
				"GAVXB7SBJRYHSG6KSQHY74N7JAFRL4PFVZCNWW2ARI6ZEKNBJSMSKW7C",
			)?,
			organization_id: organization_blockdaemon.id,
		},
		Validator {
			name: create_bounded_vec("$blockdaemon3")?,
			public_key: create_bounded_vec(
				"GAYXZ4PZ7P6QOX7EBHPIZXNWY4KCOBYWJCA4WKWRKC7XIUS3UJPT6EZ4",
			)?,
			organization_id: organization_blockdaemon.id,
		},
		// LOBSTR validators
		Validator {
			name: create_bounded_vec("$lobstr1")?,
			public_key: create_bounded_vec(
				"GCFONE23AB7Y6C5YZOMKUKGETPIAJA4QOYLS5VNS4JHBGKRZCPYHDLW7",
			)?,
			organization_id: organization_lobstr.id,
		},
		Validator {
			name: create_bounded_vec("$lobstr2")?,
			public_key: create_bounded_vec(
				"GDXQB3OMMQ6MGG43PWFBZWBFKBBDUZIVSUDAZZTRAWQZKES2CDSE5HKJ",
			)?,
			organization_id: organization_lobstr.id,
		},
		Validator {
			name: create_bounded_vec("$lobstr3")?,
			public_key: create_bounded_vec(
				"GD5QWEVV4GZZTQP46BRXV5CUMMMLP4JTGFD7FWYJJWRL54CELY6JGQ63",
			)?,
			organization_id: organization_lobstr.id,
		},
		Validator {
			name: create_bounded_vec("$lobstr4")?,
			public_key: create_bounded_vec(
				"GA7TEPCBDQKI7JQLQ34ZURRMK44DVYCIGVXQQWNSWAEQR6KB4FMCBT7J",
			)?,
			organization_id: organization_lobstr.id,
		},
		Validator {
			name: create_bounded_vec("$lobstr5")?,
			public_key: create_bounded_vec(
				"GA5STBMV6QDXFDGD62MEHLLHZTPDI77U3PFOD2SELU5RJDHQWBR5NNK7",
			)?,
			organization_id: organization_lobstr.id,
		},
		// Public Node validators
		Validator {
			name: create_bounded_vec("$hercules")?,
			public_key: create_bounded_vec(
				"GBLJNN3AVZZPG2FYAYTYQKECNWTQYYUUY2KVFN2OUKZKBULXIXBZ4FCT",
			)?,
			organization_id: organization_public_node.id,
		},
		Validator {
			name: create_bounded_vec("$boötes")?,
			public_key: create_bounded_vec(
				"GCVJ4Z6TI6Z2SOGENSPXDQ2U4RKH3CNQKYUHNSSPYFPNWTLGS6EBH7I2",
			)?,
			organization_id: organization_public_node.id,
		},
		Validator {
			name: create_bounded_vec("$lyra")?,
			public_key: create_bounded_vec(
				"GCIXVKNFPKWVMKJKVK2V4NK7D4TC6W3BUMXSIJ365QUAXWBRPPJXIR2Z",
			)?,
			organization_id: organization_public_node.id,
		},
	];

	let organizations: Vec<OrganizationOf<T>> = vec![
		organization_satoshipay,
		organization_sdf,
		organization_wirex,
		organization_coinqvest,
		organization_blockdaemon,
		organization_lobstr,
		organization_public_node,
	];

	Ok(GenesisConfig { validators, organizations, phantom: Default::default() })
}
