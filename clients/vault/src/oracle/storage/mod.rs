pub mod impls;
pub mod traits;

pub use impls::*;

/// A handler to save the envelopes map to a file.
pub struct EnvelopesFileHandler;

/// A handler to save tx sets map to a file.
pub struct TxSetsFileHandler;

pub struct ScpArchiveStorage;

pub struct TransactionsArchiveStorage;
