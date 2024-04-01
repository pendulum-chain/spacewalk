use stellar_relay_lib::sdk::SecretKey;
use wallet::keys::{get_source_secret_key_from_env, get_dest_secret_key_from_env};

pub fn random_stellar_relay_config(is_mainnet: bool) -> stellar_relay_lib::StellarOverlayConfig {
	use rand::seq::SliceRandom;

	let (stellar_node_points, dir) = stellar_relay_config_choices(is_mainnet);

	let node_point = stellar_node_points
		.choose(&mut rand::thread_rng())
		.expect("should return a value");

	stellar_relay_config_abs_path(dir, node_point)
}

pub fn specific_stellar_relay_config(
	is_mainnet: bool,
	index: usize,
) -> stellar_relay_lib::StellarOverlayConfig {
	let (stellar_node_points, dir) = stellar_relay_config_choices(is_mainnet);

	let node_point = stellar_node_points.get(index).expect("should return a value");

	stellar_relay_config_abs_path(dir, node_point)
}

fn stellar_relay_config_choices(is_mainnet: bool) -> (Vec<&'static str>, &'static str) {
	let node_points = if is_mainnet {
		vec!["frankfurt", "iowa", "singapore"]
	} else {
		vec!["sdftest1", "sdftest2", "sdftest3"]
	};

	let dir = if is_mainnet { "mainnet" } else { "testnet" };
	(node_points, dir)
}
fn stellar_relay_config_abs_path(
	dir: &str,
	node_point: &str,
) -> stellar_relay_lib::StellarOverlayConfig {
	let path_string = format!("./resources/config/{dir}/stellar_relay_config_{node_point}.json");

	stellar_relay_lib::StellarOverlayConfig::try_from_path(path_string.as_str())
		.expect("should be able to extract config")
}

pub fn get_secret_key_from_env(with_currency: bool, is_mainnet: bool) -> String {
	match with_currency {
		true => get_source_secret_key_from_env(is_mainnet),
		false => get_dest_secret_key_from_env(is_mainnet),
	}
}

pub fn get_random_secret_key() -> String {
	// Generate a new random Stellar keypair
	let secret = SecretKey::from_binary(rand::random());
	let secret_encoded = secret.to_encoding();
	// Convert the secret key to a string
	let secret_string = std::str::from_utf8(&secret_encoded).expect("Failed to convert to string");

	secret_string.to_string()
}
