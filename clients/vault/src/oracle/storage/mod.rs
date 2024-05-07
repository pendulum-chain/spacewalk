pub mod impls;
pub mod traits;

/// A handler to save the envelopes map to a file.
pub struct EnvelopesFileHandler;

/// A handler to save tx sets map to a file.
pub struct TxSetsFileHandler;

pub struct ScpArchiveStorage(pub(crate) String);

pub struct TransactionsArchiveStorage(pub(crate) String);
