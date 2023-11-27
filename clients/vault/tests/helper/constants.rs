use primitives::CurrencyId;
use std::time::Duration;
use wallet::types::PagingToken;

pub const TIMEOUT: Duration = Duration::from_secs(60);

// Be careful when changing these values because they are used in the parachain genesis config
// and only for some combination of them, secure collateralization thresholds are set.
pub const DEFAULT_TESTING_CURRENCY: CurrencyId = CurrencyId::XCM(0);
pub const DEFAULT_WRAPPED_CURRENCY_STELLAR_TESTNET: CurrencyId = CurrencyId::AlphaNum4(
	*b"USDC",
	[
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
);

const DEFAULT_WRAPPED_CURRENCY_STELLAR_MAINNET: CurrencyId = CurrencyId::AlphaNum4(
	*b"USDC",
	[
		59, 153, 17, 56, 14, 254, 152, 139, 160, 168, 144, 14, 177, 207, 228, 79, 54, 111, 125,
		190, 148, 107, 237, 7, 114, 64, 247, 246, 36, 223, 21, 197,
	],
);

pub const LESS_THAN_4_CURRENCY_CODE: CurrencyId = CurrencyId::AlphaNum4(
	*b"MXN\0",
	[
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
);

#[allow(dead_code)]
pub const DEFAULT_MAINNET_DEST_SECRET_KEY: &'static str =
	"SCJ7XV73Q642EPMUMSPO5ECOXWTMWR52MGPMWT6ELV3VUFPH653IOEUS";
pub const DEFAULT_TESTNET_DEST_SECRET_KEY: &'static str =
	"SA77KS7EHYNOO6VIT3RBH36WSWDIA4PUV53EDEDAST3OWEKMDZ5HCUGW";

#[allow(dead_code)]
pub const LAST_KNOWN_CURSOR: PagingToken = 4810432091004928;
