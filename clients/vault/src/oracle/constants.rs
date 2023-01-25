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

pub const ARCHIVE_NODE_LEDGER_BATCH: u32 = 64;

pub const STELLAR_HISTORY_BASE_URL: &str =
	"http://history.stellar.org/prd/core-live/core_live_002/scp/";

pub const STELLAR_HISTORY_BASE_URL_TRANSACTIONS: &str =
	"http://history.stellar.org/prd/core-live/core_live_002/transactions/";

/// Returns the minimum amount of SCP messages that are required to build a valid proof per network
pub fn get_min_externalized_messages(public_network: bool) -> usize {
	let validator_count =
		if public_network { VALIDATOR_COUNT_PUBLIC_NETWORK } else { VALIDATOR_COUNT_TEST_NETWORK };
	// Return 2/3 of the validator count as minimum amount
	// This value is likely higher than the actual minimum but it's a good approximation
	validator_count * 2 / 3
}

/// --- Default values for the stellar overlay connection ---
pub const OVERLAY_VERSION_PUBNET: u32 = 26;
pub const MIN_OVERLAY_VERSION_PUBNET: u32 = 23;
pub const LEDGER_VERSION_PUBNET: u32 = 19;
pub const VERSION_STRING_PUBNET: &str =
	"stellar-core 19.7.0.rc1 (7249363c60e7ddf796187149f6a236f8ad244b2b)";
// For SatoshiPay (DE, Frankfurt)
pub const TIER_1_NODE_IP_PUBNET: &str = "15.235.11.99";
pub const TIER_1_NODE_PORT_PUBNET: u32 = 11625;

pub const OVERLAY_VERSION_TESTNET: u32 = 27;
pub const MIN_OVERLAY_VERSION_TESTNET: u32 = 24;
pub const LEDGER_VERSION_TESTNET: u32 = 19;
pub const VERSION_STRING_TESTNET: &str =
	"stellar-core 19.7.0.rc1 (7249363c60e7ddf796187149f6a236f8ad244b2b)";
// For sdftest-1
pub const TIER_1_NODE_IP_TESTNET: &str = "34.235.168.98";
pub const TIER_1_NODE_PORT_TESTNET: u32 = 11625;
