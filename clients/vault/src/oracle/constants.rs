use stellar_relay::sdk::types::Uint64;

/// This is for `EnvelopesMap`; how many slots is accommodated per file.
/// This is used to compare against the length of the "keys",
/// NOT the "values" of the map
pub const MAX_SLOTS_PER_FILE: usize = 200;

/// This is for `TxHashMap`.
/// When the map reaches the MAX or more, then we write to file.
pub const MAX_TXS_PER_FILE: Uint64 = 10_000_000;

/// This is for `TxSetMap`.
pub const MAX_TXSETS_PER_FILE: usize = 20000;

pub const VALIDATOR_COUNT_TEST_NETWORK: usize = 3;
pub const VALIDATOR_COUNT_PUBLIC_NETWORK: usize = 23;

/// Returns the minimum amount of SCP messages that are required to build a valid proof per network
pub fn get_min_externalized_messages(public_network: bool) -> usize {
	let validator_count =
		if public_network { VALIDATOR_COUNT_PUBLIC_NETWORK } else { VALIDATOR_COUNT_TEST_NETWORK };
	// Return 2/3 of the validator count as minimum amount
	// This value is likely higher than the actual minimum but it's a good approximation
	validator_count * 2 / 3
}
