use primitives::CurrencyId;

// These constants best are set to match the definitions in the testchain's runtime
pub const DEFAULT_COLLATERAL_CURRENCY: CurrencyId = CurrencyId::XCM(0);
pub const DEFAULT_NATIVE_CURRENCY: CurrencyId = CurrencyId::Native;

// We support many different wrapped currencies but here we return some wrapped currency id for
// convenience in tests
pub const DEFAULT_WRAPPED_CURRENCY: CurrencyId = CurrencyId::AlphaNum4(
	*b"USDC",
	[
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
);

pub const DEFAULT_WRAPPED_CURRENCY_2: CurrencyId = CurrencyId::AlphaNum4(
	*b"USDT",
	[
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
);
pub const DEFAULT_WRAPPED_CURRENCY_3: CurrencyId = CurrencyId::AlphaNum4(
	*b"MXN\0",
	[
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
);

pub const DEFAULT_WRAPPED_CURRENCY_4: CurrencyId = CurrencyId::AlphaNum4(
	*b"ARST",
	[
		44, 123, 1, 49, 176, 55, 23, 123, 171, 123, 54, 155, 16, 50, 30, 226, 155, 231, 46, 199, 1,
		11, 4, 144, 240, 123, 51, 33, 72, 34, 159, 33,
	],
);
