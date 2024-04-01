use dotenv::dotenv;
use std::env;
// Gets env variables with precedence from the system's environment and then from the .env file.
// If one variable is not on the systme's environment, and the .env is defined, then all variables 
// will be overridden by those on the .env file.
fn get_env_variables(key: &str) -> Option<String> {
    match env::var(key) {
        Ok(value) => Some(value),
        Err(_) => {
            if dotenv::from_filename("../vault/resources/secretkey/.env").is_ok() {
                env::var(key).ok()
            } else {
                None
            }
        }
    }
}

pub fn get_dest_secret_key_from_env(is_mainnet: bool)-> String{
	let maybe_secret = match is_mainnet {
		true => get_env_variables("DEST_SECRET_MAINNET").expect("should return a string"),
		false => get_env_variables("DEST_SECRET_TESTNET").expect("should return a string"),
	};

	maybe_secret
}

pub fn get_source_secret_key_from_env(is_mainnet: bool)-> String{
	let maybe_secret = match is_mainnet {
		true => get_env_variables("SOURCE_SECRET_MAINNET").expect("should return a string"),
		false => get_env_variables("SOURCE_SECRET_TESTNET").expect("should return a string"),
	};

	maybe_secret
}
