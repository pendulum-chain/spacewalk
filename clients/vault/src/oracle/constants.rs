use stellar_relay::sdk::types::Uint64;

/// This is for `EnvelopesMap`; how many slots is accommodated per file.
/// This is used to compare against the length of the "keys",
/// NOT the "values" of the map
pub const MAX_SLOTS_PER_FILE: usize = 200;

/// This is for `EnvelopesMap`. Make sure that we have a minimum set of envelopes per slot,
/// before writing to file.
pub const MIN_EXTERNALIZED_MESSAGES: usize = 15;

/// This is for `TxHashMap`.
/// When the map reaches the MAX or more, then we write to file.
pub const MAX_TXS_PER_FILE: Uint64 = 10_000_000;

/// This is for `TxSetMap`.
pub const MAX_TXSETS_PER_FILE: usize = 20000;