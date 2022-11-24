#![allow(dead_code)]

mod collector;
mod constants;
mod errors;
mod handler;
pub mod storage;
mod types;

pub use collector::{Proof, ProofStatus};
pub use handler::*;
pub use storage::prepare_directories;

use collector::*;
use errors::Error;
pub use storage::*;
use types::*;

/// A filter trait to check whether `T` should be processed.
pub trait FilterWith<T> {
	/// unique identifier of this filter
	fn name(&self) -> &'static str;

	/// logic to check whether a given param should be processed.
	fn check_for_processing(&self, param: &T) -> bool;
}
