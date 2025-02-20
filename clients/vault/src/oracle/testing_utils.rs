use rand;
use stellar_relay_lib::sdk::SecretKey;
pub fn random_stellar_relay_config(is_mainnet: bool) -> stellar_relay_lib::StellarOverlayConfig {
	let (_, dir) = stellar_relay_config_choices(is_mainnet);

	let node_point = if is_mainnet { "mainnet" } else { "sdftest" };
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
		vec!["iowa", "singapore", "frankfurt"]
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

pub fn get_random_secret_key() -> String {
	// Generate a new random Stellar keypair
	let secret = SecretKey::from_binary(rand::random());
	let secret_encoded = secret.to_encoding();
	// Convert the secret key to a string
	let secret_string = std::str::from_utf8(&secret_encoded).expect("Failed to convert to string");

	secret_string.to_string()
}
