use crate::stellar_oracle::types::Slot;
use stellar_relay::sdk::types::Uint64;

/// This is for `EnvelopesMap`; how many slots is accommodated per file.
pub const MAX_SLOTS_PER_FILE: Slot = 200;

/// This is for `EnvelopesMap`. Make sure that we have a minimum set of envelopes per slot,
/// before writing to file.
pub const MIN_EXTERNALIZED_MESSAGES: usize = 15;

/// This is both for `TxSetMap` and `TxHashMap`.
/// When the map reaches the MAX or more, then we write to file.
pub const MAX_TXS_PER_FILE: Uint64 = 10_000_000;

pub const VAULT_ADDRESSES_FILTER: &[&str] = &["GAP4SFKVFVKENJ7B7VORAYKPB3CJIAJ2LMKDJ22ZFHIAIVYQOR6W3CXF"];

pub const TIER_1_VALIDATOR_IP_TESTNET: &str = "34.235.168.98";
pub const TIER_1_VALIDATOR_IP_PUBLIC: &str = "135.181.16.110";
