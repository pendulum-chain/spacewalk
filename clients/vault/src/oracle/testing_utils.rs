use stellar_relay_lib::sdk::SecretKey;

pub fn get_test_stellar_relay_config(is_mainnet: bool) -> stellar_relay_lib::StellarOverlayConfig {
	use rand::seq::SliceRandom;

	let stellar_node_points: Vec<&str> = if is_mainnet {
		vec!["frankfurt", "iowa", "singapore"]
	} else {
		vec!["sdftest1", "sdftest2", "sdftest3"]
	};
	let dir = if is_mainnet { "mainnet" } else { "testnet" };

	let res = stellar_node_points
		.choose(&mut rand::thread_rng())
		.expect("should return a value");
	let path_string = format!("./resources/config/{dir}/stellar_relay_config_{res}.json");

	stellar_relay_lib::StellarOverlayConfig::try_from_path(path_string.as_str())
		.expect("should be able to extract config")
}

pub fn get_test_secret_key(is_mainnet: bool) -> String {
	let file_name = if is_mainnet { "mainnet" } else { "testnet" };
	let path = format!("./resources/secretkey/stellar_secretkey_{file_name}");
	std::fs::read_to_string(path).expect("should return a string")
}

pub fn get_random_secret_key() -> String {
	// Generate a new random Stellar keypair
	let secret = SecretKey::from_binary(rand::random());
	let secret_encoded = secret.to_encoding();
	// Convert the secret key to a string
	let secret_string = std::str::from_utf8(&secret_encoded).expect("Failed to convert to string");

	secret_string.to_string()
}
