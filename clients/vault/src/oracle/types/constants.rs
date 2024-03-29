use wallet::Slot;

/// This is for `EnvelopesMap`; how many slots is accommodated per file.
/// This is used to compare against the length of the "keys",
/// NOT the "values" of the map
pub const MAX_SLOTS_PER_FILE: usize = 200;

/// A default limit of number of items the `TxSetMap` and `EnvelopesMap` can hold before
/// dropping some.
pub const DEFAULT_MAX_ITEMS_IN_QUEUE: usize = MAX_SLOTS_PER_FILE;

// the maximum distance of the selected slot from the current slot.
// this is primarily used when deciding to move maps to a file.
pub const MAX_DISTANCE_FROM_CURRENT_SLOT: Slot = 3;

pub const VALIDATOR_COUNT_TEST_NETWORK: usize = 3;
pub const VALIDATOR_COUNT_PUBLIC_NETWORK: usize = 23;

/// The default setting for `MAX_SLOTS_TO_REMEMBER` is 12 Slots, ie. 72 seconds.
/// See [here](https://github.com/stellar/stellar-core/blob/ca2fb06059c15442cb4c9a8c89de1a8fc3579a39/docs/stellar-core_example.cfg#L478)
pub const MAX_SLOTS_TO_REMEMBER: Slot = 12;

pub const ARCHIVE_NODE_LEDGER_BATCH: Slot = 64;

/// Returns the minimum amount of SCP messages that are required to build a valid proof per network
pub fn get_min_externalized_messages(public_network: bool) -> usize {
	let validator_count =
		if public_network { VALIDATOR_COUNT_PUBLIC_NETWORK } else { VALIDATOR_COUNT_TEST_NETWORK };
	// Return 2/3 of the validator count as minimum amount
	// This value is likely higher than the actual minimum but it's a good approximation
	validator_count * 2 / 3
}
