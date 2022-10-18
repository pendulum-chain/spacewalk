use crate::{
	error::Error,
	horizon::{
		BalanceConversion, CurrencyConversion, CurrencyId, HorizonAccountResponse,
		StringCurrencyConversion,
	},
};
use runtime::{Balance, RedeemEvent, SpacewalkParachain};
use service::{spawn_cancelable, Error as ServiceError, ShutdownSender};
use sp_runtime::traits::{Convert, StaticLookup};
use sp_std::{
	convert::{From, TryInto},
	prelude::*,
	str,
};
use stellar::{network::TEST_NETWORK, PublicKey, SecretKey, XdrCodec};
use stellar_relay::sdk as stellar;
use stellar_relay::sdk::types::Preconditions;

const SUBMISSION_TIMEOUT_PERIOD: u64 = 10000;

fn is_vault(vault_key: &String, public_key: [u8; 32]) -> bool {
	let vault_keypair: SecretKey = SecretKey::from_encoding(vault_key).unwrap();
	return public_key == *vault_keypair.get_public().as_binary()
}

async fn fetch_from_remote(request_url: &str) -> Result<HorizonAccountResponse, Error> {
	tracing::info!("Sending request to: {}", request_url);

	let client = reqwest::Client::new();

	let response = client
		.get(request_url)
		.send()
		.await
		.map_err(|e| {
			tracing::error!("Error while sending request: {}", e);
			Error::HttpFetchingError
		})?
		.json::<HorizonAccountResponse>()
		.await
		.map_err(|e| {
			tracing::error!("Error while sending request: {}", e);
			Error::HttpFetchingError
		})?;

	Ok(response)
}

async fn submit_transaction_to_horizon(
	transaction_envelope: &stellar::TransactionEnvelope,
	timeout_milliseconds: u64,
	testnet: bool,
) -> Result<(), Error> {
	let horizon_url = if testnet {
		"https://horizon-testnet.stellar.org/transactions"
	} else {
		"https://horizon.stellar.org/transactions"
	};

	let envelope_base64 = transaction_envelope.to_base64_xdr();
	let xdr_string = str::from_utf8(&envelope_base64)?;
	tracing::info!(
		"Submitting transaction to Stellar network: {}, tx_xdr: {:?}",
		horizon_url,
		&xdr_string
	);
	let params = [("tx", xdr_string)];

	let builder = reqwest::ClientBuilder::new();
	let client = builder
		.timeout(std::time::Duration::from_millis(timeout_milliseconds))
		.build()
		.unwrap();

	let response = client.post(horizon_url).form(&params).send().await;

	match response {
		Ok(response) => {
			tracing::info!("Transaction submitted to Stellar network: {:?}", response);
			let status = response.status();
			let response_text = &response.text().await.map_err(|_| Error::HttpPostError)?;
			if status != reqwest::StatusCode::OK {
				tracing::error!("Transaction submission failed: {:?}", response_text);
				return Err(Error::HttpPostError)
			}
			Ok(())
		},
		Err(e) => {
			tracing::error!("Error submitting transaction to Stellar network: {}", e);
			Err(Error::HttpPostError)
		},
	}
}

async fn fetch_latest_seq_no(stellar_addr: &str) -> Result<u64, Error> {
	let request_url = String::from("https://horizon-testnet.stellar.org/accounts/") + stellar_addr;

	let horizon_response = fetch_from_remote(request_url.as_str()).await.map_err(|e| {
		tracing::error!("fetch_latest_seq_no error: {:?}", e);
		Error::HttpFetchingError
	})?;

	String::from_utf8(horizon_response.sequence)
		.map(|string| string.parse::<u64>().unwrap())
		.map_err(|_| Error::SeqNoParsingError)
}

fn sign_stellar_tx(
	tx: stellar::types::Transaction,
	secret_key: SecretKey,
) -> Result<stellar::TransactionEnvelope, Error> {
	let mut envelope = tx.into_transaction_envelope();
	envelope.sign(&TEST_NETWORK, vec![&secret_key])?;

	Ok(envelope)
}

async fn submit_stellar_tx(tx: stellar::TransactionEnvelope) -> Result<(), Error> {
	let mut last_error: Option<Error> = None;

	for attempt in 1..=3 {
		tracing::debug!("Attempt #{} to submit Stellar transaction…", attempt);

		let response = submit_transaction_to_horizon(&tx, SUBMISSION_TIMEOUT_PERIOD, true).await;

		match response {
			Ok(()) => return Ok(()),
			Err(error) => {
				last_error = Some(error);
			},
		}
	}

	// Can only panic if no submission was ever attempted
	Err(last_error.unwrap())
}

fn create_withdrawal_tx(
	vault_secret_key: &String,
	stellar_addr: &stellar::PublicKey,
	seq_num: i64,
	asset: stellar::Asset,
	amount: Balance,
) -> Result<stellar::Transaction, Error> {
	let destination_addr = stellar_addr.as_binary();

	let source_keypair: SecretKey = SecretKey::from_encoding(vault_secret_key).unwrap();

	let source_pubkey = source_keypair.get_public().clone();
	let mut tx = stellar::Transaction::new(
		source_pubkey,
		seq_num,
		Some(10_000),
		Preconditions::PrecondNone,
		None,
	)?;

	let destination = stellar::MuxedAccount::KeyTypeEd25519(*destination_addr);
	let amount = stellar::StroopAmount(
		BalanceConversion::lookup(amount).map_err(|_| Error::BalanceConversionError)?,
	);

	let operation = stellar::Operation::new_payment(destination, asset, amount)?;

	tx.append_operation(operation)?;

	Ok(tx)
}

async fn execute_withdrawal(
	vault_secret_key: &String,
	amount: Balance,
	currency_id: CurrencyId,
	destination_stellar_address: PublicKey,
) -> Result<(), Error> {
	let asset = CurrencyConversion::lookup(currency_id)?;

	let vault_keypair: SecretKey = match SecretKey::from_encoding(vault_secret_key) {
		Ok(keypair) => keypair,
		Err(e) => {
			tracing::error!("Error while creating keypair from secret key: {:?}", e);
			return Err(Error::StellarSdkError)
		},
	};
	let vault_encoded = vault_keypair.get_public().to_encoding().clone();
	let vault_address = str::from_utf8(vault_encoded.as_slice())?;

	let seq_no = fetch_latest_seq_no(vault_address).await.map(|seq_no| seq_no + 1)?;
	let transaction = create_withdrawal_tx(
		vault_secret_key,
		&destination_stellar_address,
		seq_no as i64,
		asset,
		amount,
	)?;
	let signed_envelope = sign_stellar_tx(transaction, vault_keypair)?;

	let result = submit_stellar_tx(signed_envelope).await;

	match result {
		Ok(()) => {
			tracing::info!(
				"✔️  Successfully submitted withdrawal transaction to Stellar, crediting {}",
				str::from_utf8(destination_stellar_address.to_encoding().as_slice()).unwrap()
			);
			Ok(())
		},
		Err(error) => {
			tracing::error!("Withdrawal transaction submission failed: {:?}", error);
			Err(error)
		},
	}
}

pub async fn listen_for_redeem_requests(
	shutdown_tx: ShutdownSender,
	parachain_rpc: SpacewalkParachain,
	vault_secret_key: String,
) -> Result<(), ServiceError> {
	tracing::info!("Starting to listen for redeem requests…");
	parachain_rpc
		.on_event::<RedeemEvent, _, _, _>(
			|event| async {
				tracing::info!("Received redeem request: {:?}", event);

				// within this event callback, we captured the arguments of
				// listen_for_redeem_requests by reference. Since spawn requires static lifetimes,
				// we will need to capture the arguments by value rather than by reference, so clone
				// these:
				let secret_key = vault_secret_key.clone();
				let asset_code = event.asset_code.clone();
				let asset_issuer = event.asset_issuer.clone();
				let stellar_user_id = event.stellar_user_id.clone();
				let stellar_vault_id = event.stellar_vault_id.clone();
				let amount = event.amount.clone();

				if !is_vault(&vault_secret_key, stellar_vault_id.try_into().unwrap()) {
					tracing::info!(
						"Rejecting redeem request: not an vault. Redeem was for {}",
						str::from_utf8(
							stellar::PublicKey::from_binary(stellar_vault_id)
								.to_encoding()
								.as_slice()
						)
						.unwrap()
					);
					return
				}

				// Spawn a new task so that we handle these events concurrently
				spawn_cancelable(shutdown_tx.subscribe(), async move {
					tracing::info!("Executing redeem {:?}", event);

					let currency_id = match StringCurrencyConversion::convert((
						asset_code.clone(),
						asset_issuer.clone(),
					)) {
						Ok(currency_id) => currency_id,
						Err(_) => {
							tracing::error!(
								"Error while converting currency: {:?}",
								(asset_code, asset_issuer)
							);
							return
						},
					};
					let destination_stellar_address =
						stellar::PublicKey::from_binary(stellar_user_id);

					let result = execute_withdrawal(
						&secret_key,
						amount,
						currency_id,
						destination_stellar_address,
					)
					.await;

					match result {
						Ok(_) =>
							tracing::info!("Completed redeem request with amount {}", event.amount),
						Err(e) =>
							tracing::error!("Failed to process redeem request: {}", e.to_string()),
					}
				});
			},
			|error| tracing::error!("Error reading redeem event: {}", error.to_string()),
		)
		.await?;
	Ok(())
}

#[cfg(all(test, feature = "standalone-metadata"))]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	const STELLAR_VAULT_SECRET_KEY: &str =
		"SB6WHKIU2HGVBRNKNOEOQUY4GFC4ZLG5XPGWLEAHTIZXBXXYACC76VSQ";
	const DESTINATION_PUBLIC_KEY: &str = "GA6ZDMRVBTHIISPVD7ZRCVX6TWDXBOH2TE5FAADJXZ52YL4GCFI4HOHU";

	#[tokio::test(flavor = "multi_thread")]
	async fn test_execute_withdrawal() {
		let amount = 100000000;
		let currency_id = StringCurrencyConversion::convert((
			"USDC".as_bytes().to_vec(),
			"GAKNDFRRWA3RPWNLTI3G4EBSD3RGNZZOY5WKWYMQ6CQTG3KIEKPYWAYC".as_bytes().to_vec(),
		))
		.unwrap();
		let destination_stellar_address =
			stellar_relay::sdk::PublicKey::from_encoding(DESTINATION_PUBLIC_KEY).unwrap();

		let result = execute_withdrawal(
			&STELLAR_VAULT_SECRET_KEY.to_string(),
			amount,
			currency_id,
			destination_stellar_address,
		)
		.await;
		assert_ok!(result);
	}
}
