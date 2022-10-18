#![allow(dead_code)]

mod collector;
mod constants;
mod errors;
mod handler;
mod storage;
mod types;

#[cfg(test)]
mod tests;


pub use handler::*;
pub use storage::prepare_directories;

use collector::*;
use constants::*;
use errors::Error;
use stellar_relay::{node::NodeInfo, ConnConfig};
use storage::*;
use types::*;

// a filter trait to get only the ones that makes sense to the structure.
pub trait TxHandler<T> {
    fn process_tx(&self, param: &T) -> bool;
}

#[derive(Copy, Clone)]
pub struct FilterTx;
