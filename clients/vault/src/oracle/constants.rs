use stellar_relay_lib::sdk::types::Uint64;

use crate::oracle::types::Slot;

/// This is for `EnvelopesMap`; how many slots is accommodated per file.
/// This is used to compare against the length of the "keys",
/// NOT the "values" of the map
pub const MAX_SLOTS_PER_FILE: usize = 200;

/// This is used to determine how many items the `TxSetMap` and `EnvelopesMap` can hold before
/// dropping some.
pub const MAX_ITEMS_IN_QUEUE: usize = MAX_SLOTS_PER_FILE;

// the maximum distance of the selected slot from the current slot.
// this is primarily used when deciding to move maps to a file.
pub const MAX_DISTANCE_FROM_CURRENT_SLOT: Slot = 3;

pub const VALIDATOR_COUNT_TEST_NETWORK: usize = 3;
pub const VALIDATOR_COUNT_PUBLIC_NETWORK: usize = 23;

pub const MAX_SLOT_TO_REMEMBER: u64 = 12;

pub const STELLAR_HISTORY_BASE_URL: &str =
	"http://history.stellar.org/prd/core-live/core_live_002/scp/";

/// Returns the minimum amount of SCP messages that are required to build a valid proof per network
pub fn get_min_externalized_messages(public_network: bool) -> usize {
	let validator_count =
		if public_network { VALIDATOR_COUNT_PUBLIC_NETWORK } else { VALIDATOR_COUNT_TEST_NETWORK };
	// Return 2/3 of the validator count as minimum amount
	// This value is likely higher than the actual minimum but it's a good approximation
	validator_count * 2 / 3
}

/// --- Default values for the stellar overlay connection ---
pub const LEDGER_VERSION_PUBNET: u32 = 26;
pub const OVERLAY_VERSION_PUBNET: u32 = 23;
pub const MIN_OVERLAY_VERSION_PUBNET: u32 = 19;
pub const VERSION_STRING_PUBNET: &str = "v19.6.0";
// For SatoshiPay (DE, Frankfurt)
pub const TIER_1_NODE_IP_PUBNET: &str = "141.95.47.112";
pub const TIER_1_NODE_PORT_PUBNET: u32 = 11625;

pub const LEDGER_VERSION_TESTNET: u32 = 26;
pub const OVERLAY_VERSION_TESTNET: u32 = 23;
pub const MIN_OVERLAY_VERSION_TESTNET: u32 = 19;
pub const VERSION_STRING_TESTNET: &str =
	"stellar-core 19.6.0 (b3a6bc28116e80bff7889c2f3bcd7c30dd1ac4d6)";
// For sdftest-1
pub const TIER_1_NODE_IP_TESTNET: &str = "34.235.168.98";
pub const TIER_1_NODE_PORT_TESTNET: u32 = 11625;
