#![allow(dead_code)]

mod collector;
mod constants;
mod errors;
mod handler;
mod storage;
mod types;

pub use collector::Proof;
pub use handler::*;
pub use storage::prepare_directories;

use collector::*;
use errors::Error;
use storage::*;
use types::*;

/// A filter trait to check whether `T` should be processed.
pub trait FilterWith<T> {
	/// unique identifier of this filter
	fn name(&self) -> FilterName;

	/// logic to check whether a given param should be processed.
	fn check_for_processing(&self, param: &T) -> bool;
}
