#[allow(unused_imports)]
use dotenv::dotenv;
use std::env;

// Gets all environment variables contained in the local .env file and returns
// the given variable by key if available. This will override the variables
// previously passed to the environment.
// A variable not .env but passed to the environment will not be overridden.
fn get_env_variables(key: &str) -> Option<String> {
    dotenv::from_filename("../.env").ok();
    env::var(key).ok()
}

pub fn get_dest_secret_key_from_env(is_mainnet: bool)-> String{
	let maybe_secret = match is_mainnet {
		true => get_env_variables("DEST_STELLAR_SECRET_MAINNET").expect("Failed to read secret key from environment"),
		false => get_env_variables("DEST_STELLAR_SECRET_TESTNET").expect("Failed to read secret key from environment"),
	};

	maybe_secret
}

pub fn get_source_secret_key_from_env(is_mainnet: bool)-> String{
	let maybe_secret = match is_mainnet {
		true => get_env_variables("SOURCE_STELLAR_SECRET_MAINNET").expect("Failed to read secret key from environment"),
		false => get_env_variables("SOURCE_STELLAR_SECRET_TESTNET").expect("Failed to read secret key from environment"),
	};

	maybe_secret
}
