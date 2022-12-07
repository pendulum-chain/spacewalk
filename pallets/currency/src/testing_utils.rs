use primitives::{CurrencyId, CurrencyId::Token, DOT, PEN};

// These constants best are set to match the definitions in the testchain's runtime
pub const DEFAULT_COLLATERAL_CURRENCY: CurrencyId = Token(DOT);
pub const DEFAULT_NATIVE_CURRENCY: CurrencyId = Token(PEN);

// Return some wrapped currency id for convenience in tests
pub const DEFAULT_WRAPPED_CURRENCY: CurrencyId = CurrencyId::AlphaNum4 {
	code: *b"USDC",
	issuer: [
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
};

pub fn get_wrapped_currency_id() -> CurrencyId {
	DEFAULT_WRAPPED_CURRENCY
}
