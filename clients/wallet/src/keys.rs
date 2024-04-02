#[allow(unused_imports)]
use dotenv::dotenv;
use std::env;
// Gets env variables with precedence from the system environment and then from the .env file.
// If one variable is not on the system environment, and the .env is defined, then all variables 
// will be overridden by those on the .env file.
fn get_env_variables(key: &str) -> Option<String> {
    dotenv::from_filename("../vault/resources/secretkey/.env").ok();
    env::var(key).ok()
}

pub fn get_dest_secret_key_from_env(is_mainnet: bool)-> String{
	let maybe_secret = match is_mainnet {
		true => get_env_variables("DEST_SECRET_MAINNET").expect("Failed to read secret key from environment"),
		false => get_env_variables("DEST_SECRET_TESTNET").expect("Failed to read secret key from environment"),
	};

	maybe_secret
}

pub fn get_source_secret_key_from_env(is_mainnet: bool)-> String{
	let maybe_secret = match is_mainnet {
		true => get_env_variables("SOURCE_SECRET_MAINNET").expect("Failed to read secret key from environment"),
		false => get_env_variables("SOURCE_SECRET_TESTNET").expect("Failed to read secret key from environment"),
	};

	maybe_secret
}
