#![allow(dead_code)]

pub use agent::*;
pub use collector::Proof;
use collector::*;
pub use errors::Error;
pub use storage::*;
use types::*;

mod agent;
mod collector;
mod constants;
mod errors;
pub mod storage;
pub mod types;

#[cfg(test)]
pub fn test_stellar_relay_config() -> stellar_relay_lib::StellarOverlayConfig {
	use rand::seq::SliceRandom;

	let stellar_node_points: Vec<&str> = vec!["iowa", "frankfurt", "singapore"];

	let res = stellar_node_points
		.choose(&mut rand::thread_rng())
		.expect("should return a value");
	let path_string = format!("./resources/config/mainnet/stellar_relay_config_mainnet_{res}.json");

	stellar_relay_lib::StellarOverlayConfig::try_from_path(path_string.as_str())
		.expect("should be able to extract config")
}

#[cfg(test)]
pub fn test_secret_key() -> String {
	let path = "./resources/secretkey/stellar_secretkey_mainnet";
	std::fs::read_to_string(path).expect("should return a string")
}
