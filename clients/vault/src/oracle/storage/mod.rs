mod impls;
pub mod traits;

pub use impls::*;

/// A handler to save the envelopes map to a file.
pub struct EnvelopesFileHandler;

/// A handler to save tx sets map to a file.
pub struct TxSetsFileHandler;


/// A handler to save the tx hashes to a file.
/// TODO: This is currently not being used,
/// since it's still under discussion about how to lookup through files.
pub struct TxHashesFileHandler;
