//todo: remove
use crate::{
	error::Error,
	horizon::{HorizonTransactionsResponse, Transaction},
};
use async_trait::async_trait;
use runtime::SpacewalkPallet;
use service::Error as ServiceError;
use sp_std::{convert::From, str, vec::Vec};
use std::time::Duration;
use stellar::SecretKey;
use stellar_relay_lib::sdk as stellar;
use tokio::time::sleep;

const POLL_INTERVAL: u64 = 5000;

#[async_trait]
pub trait HorizonClient {
	async fn get_transactions(&self, url: &str) -> Result<HorizonTransactionsResponse, Error>;
}

#[async_trait]
impl HorizonClient for reqwest::Client {
	async fn get_transactions(&self, url: &str) -> Result<HorizonTransactionsResponse, Error> {
		let response = self
			.get(url)
			.send()
			.await
			.map_err(|_| Error::HttpFetchingError)?
			.json::<HorizonTransactionsResponse>()
			.await
			.map_err(|_| Error::HttpFetchingError)?;

		Ok(response)
	}
}

struct HorizonFetcher<P: SpacewalkPallet, C: HorizonClient> {
	parachain_rpc: P,
	client: C,
	vault_secret_key: String,
	last_tx_id: Option<Vec<u8>>,
}

impl<P: SpacewalkPallet, C: HorizonClient> HorizonFetcher<P, C> {
	pub fn new(parachain_rpc: P, client: C, vault_secret_key: String) -> Self {
		Self { parachain_rpc, client, vault_secret_key, last_tx_id: None }
	}

	/// Fetch recent transactions from remote and deserialize to HorizonResponse
	/// Since the limit in the request url is set to one it will always fetch just one
	async fn fetch_latest_txs(&self) -> Result<HorizonTransactionsResponse, Error> {
		let vault_keypair: SecretKey = SecretKey::from_encoding(&self.vault_secret_key).unwrap();
		let vault_address = vault_keypair.get_public();

		let request_url = String::from("https://horizon-testnet.stellar.org/accounts/") +
			str::from_utf8(vault_address.to_encoding().as_slice())
				.map_err(|_| Error::HttpFetchingError)? +
			"/transactions?order=desc&limit=1";

		let horizon_response = self.client.get_transactions(request_url.as_str()).await;

		horizon_response
	}

	fn is_unhandled_transaction(&mut self, tx: &Transaction) -> bool {
		const UP_TO_DATE: () = ();
		let latest_tx_id_utf8 = &tx.id;

		let prev_tx_id = &self.last_tx_id;
		let initial = !matches!(prev_tx_id, Some(_));

		let result = match prev_tx_id {
			Some(prev_tx_id) =>
				if prev_tx_id == latest_tx_id_utf8 {
					Err(UP_TO_DATE)
				} else {
					Ok(latest_tx_id_utf8.clone())
				},
			None => Ok(latest_tx_id_utf8.clone()),
		};

		match result {
			Ok(latest_tx_id) => {
				self.last_tx_id = Some(latest_tx_id.clone());
				if !initial {
					tracing::info!(
                        "Found new transaction from Horizon (id {:#?}). Starting to process new transaction",
                        str::from_utf8(&latest_tx_id).unwrap()
                    );

					true
				} else {
					tracing::info!("Initial transaction handled");
					false
				}
			},
			Err(UP_TO_DATE) => {
				tracing::info!("Already up to date");
				false
			},
		}
	}

	async fn report_transaction(&self, tx: &Transaction) -> Result<(), Error> {
		// Send new transaction to spacewalk bridge pallet
		let result = self.parachain_rpc.report_stellar_transaction(&tx.envelope_xdr).await;
		match result {
			Ok(_) => Ok(()),
			Err(e) => Err(Error::RuntimeError(e)),
		}
	}

	async fn fetch_horizon_and_process_new_transactions(&mut self) {
		let res = self.fetch_latest_txs().await;
		let transactions = match res {
			Ok(txs) => txs._embedded.records,
			Err(e) => {
				tracing::warn!("Failed to fetch transactions: {:?}", e);
				return
			},
		};

		if transactions.len() > 0 {
			let tx = &transactions[0];
			if self.is_unhandled_transaction(tx) {
				let result = self.report_transaction(tx).await;
				match result {
					Ok(_) => {
						tracing::info!("Reported Stellar transaction to spacewalk pallet");
					},
					Err(e) => {
						tracing::warn!("Failed to process transaction: {:?}", e);
					},
				}
			}
		}
	}
}

pub async fn poll_horizon_for_new_transactions<P: SpacewalkPallet>(
	parachain_rpc: P,
	vault_secret_key: String,
) -> Result<(), ServiceError> {
	let horizon_client = reqwest::Client::new();
	let mut fetcher = HorizonFetcher::new(parachain_rpc, horizon_client, vault_secret_key);
	// Start polling horizon every 5 seconds
	loop {
		fetcher.fetch_horizon_and_process_new_transactions().await;

		sleep(Duration::from_millis(POLL_INTERVAL)).await;
	}
}

#[cfg(all(test, feature = "standalone-metadata"))]
mod tests {
	use super::*;
	use crate::horizon;
	use async_trait::async_trait;

	const STELLAR_VAULT_SECRET_KEY: &str =
		"SB6WHKIU2HGVBRNKNOEOQUY4GFC4ZLG5XPGWLEAHTIZXBXXYACC76VSQ";

	const TX_STRING_1: &str = r#" {
  "_links": { },
  "_embedded": {
    "records": [
      {
        "_links": { },
        "id": "b9d0b2292c4e09e8eb22d036171491e87b8d2086bf8b265874c8d182cb9c9020",
        "paging_token": "755914248192",
        "successful": true,
        "hash": "b9d0b2292c4e09e8eb22d036171491e87b8d2086bf8b265874c8d182cb9c9020",
        "ledger": 176,
        "created_at": "2022-03-16T10:02:39Z",
        "source_account": "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
        "source_account_sequence": "1",
        "fee_account": "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
        "fee_charged": "1100",
        "max_fee": "1100",
        "operation_count": 11,
        "envelope_xdr": "AAAAAgAAAABi/B0L0JGythwN1lY0aypo19NHxvLCyO5tBEcCVvwF9wAABEwAAAAAAAAAAQAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAsAAAAAAAAAAAAAAAAQfdFrLDgzSIIugR73qs8U0ZiKbwBUclTTPh5thlbgnAFjRXhdigAAAAAAAAAAAAAAAAAA3b5KF6uk1w1fSKYLrzR8gF2lB+AHAi6oU6CaWhunAskAAAAXSHboAAAAAAAAAAAAAAAAAHfmNeMLin2aTUfxa530ZRn4zwRu7ROAQfUJeJco8HSCAAHGv1JjQAAAAAAAAAAAAAAAAAAAlRt2go9sp7E1a5ZWvr7vin4UPrFQThpQax1lOFm33AAAABdIdugAAAAAAAAAAAAAAAAAmv+knlR6JR2VqWeU0k/4FgvZ/tSV5DEY4gu0iOTKgpUAAAAXSHboAAAAAAAAAAAAAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAAF0h26AAAAAABAAAAAACVG3aCj2ynsTVrlla+vu+KfhQ+sVBOGlBrHWU4WbfcAAAABgAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bf/////////8AAAABAAAAAJr/pJ5UeiUdlalnlNJP+BYL2f7UleQxGOILtIjkyoKVAAAABgAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bf/////////8AAAABAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAAAQAAAAAAlRt2go9sp7E1a5ZWvr7vin4UPrFQThpQax1lOFm33AAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAJGE5yoAAAAAABAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAAAQAAAACa/6SeVHolHZWpZ5TST/gWC9n+1JXkMRjiC7SI5MqClQAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAJGE5yoAAAAAAAAAAAAAAAAABKBB+2UBMP/abwcm/M1TXO+/JQWhPwkalgqizKmXyRIQx7qh6aAFYAAAAAAAAAAARW/AX3AAAAQDVB8fT2ZXF0PZqtZX9brK0kz+P4G8VKs1DkDklP6ULsvXRexXFBdH4xG8xRAsR1HJeEBH278hiBNNvUwNw6zgzGYc0bAAAAQLgZUU/oYGL7frWDQhJHhCQu9JmfqN03PrJq4/cJrN1OSUWXnmLc94sv8m2L+cxl2p0skr2Jxy+vt1Lcxkv7wAI4WbfcAAAAQHvZEVqlygIProf3jVTZohDWm2WUNrFAFXf1LctTqDCQBHph14Eo+APwrTURLLYTIvNoXeGzBKbL03SsOARWcQLkyoKVAAAAQHAvKv2/Ro4+cNh6bKQO/G9NNiUozYysGwG1GvJQkFjwy/OTsL6WBfuI0Oye84lVBVrQVk2EY1ERFhgdMpuFSg4=",
        "result_xdr": "AAAAAAAABEwAAAAAAAAACwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGAAAAAAAAAAAAAAAGAAAAAAAAAAAAAAABAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        "result_meta_xdr": "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",
        "fee_meta_xdr": "AAAAAgAAAAMAAAABAAAAAAAAAABi/B0L0JGythwN1lY0aypo19NHxvLCyO5tBEcCVvwF9w3gtrOnZAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAEAAACwAAAAAAAAAABi/B0L0JGythwN1lY0aypo19NHxvLCyO5tBEcCVvwF9w3gtrOnY/u0AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAA==",
        "memo_type": "none",
        "signatures": [
          "NUHx9PZlcXQ9mq1lf1usrSTP4/gbxUqzUOQOSU/pQuy9dF7FcUF0fjEbzFECxHUcl4QEfbvyGIE029TA3DrODA==",
          "uBlRT+hgYvt+tYNCEkeEJC70mZ+o3Tc+smrj9wms3U5JRZeeYtz3iy/ybYv5zGXanSySvYnHL6+3UtzGS/vAAg==",
          "e9kRWqXKAg+uh/eNVNmiENabZZQ2sUAVd/Uty1OoMJAEemHXgSj4A/CtNREsthMi82hd4bMEpsvTdKw4BFZxAg==",
          "cC8q/b9Gjj5w2HpspA78b002JSjNjKwbAbUa8lCQWPDL85OwvpYF+4jQ7J7ziVUFWtBWTYRjUREWGB0ym4VKDg=="
        ],
        "valid_after": "1970-01-01T00:00:00Z"
      }
    ]
  }
}"#;

	const TX_STRING_2: &str = r#"{
  "_links": { },
  "_embedded": {
    "records": [
      {
        "_links": { },
        "id": "0eb09b01df05990221197c67b5c7a03157008e270a690b31f3809b5083506895",
        "paging_token": "876173332480",
        "successful": true,
        "hash": "b9d0b2292c4e09e8eb22d036171491e87b8d2086bf8b265874c8d182cb9c9020",
        "ledger": 176,
        "created_at": "2022-03-16T10:02:39Z",
        "source_account": "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
        "source_account_sequence": "1",
        "fee_account": "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
        "fee_charged": "1100",
        "max_fee": "1100",
        "operation_count": 11,
        "envelope_xdr": "AAAAAgAAAABi/B0L0JGythwN1lY0aypo19NHxvLCyO5tBEcCVvwF9wAABEwAAAAAAAAAAQAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAsAAAAAAAAAAAAAAAAQfdFrLDgzSIIugR73qs8U0ZiKbwBUclTTPh5thlbgnAFjRXhdigAAAAAAAAAAAAAAAAAA3b5KF6uk1w1fSKYLrzR8gF2lB+AHAi6oU6CaWhunAskAAAAXSHboAAAAAAAAAAAAAAAAAHfmNeMLin2aTUfxa530ZRn4zwRu7ROAQfUJeJco8HSCAAHGv1JjQAAAAAAAAAAAAAAAAAAAlRt2go9sp7E1a5ZWvr7vin4UPrFQThpQax1lOFm33AAAABdIdugAAAAAAAAAAAAAAAAAmv+knlR6JR2VqWeU0k/4FgvZ/tSV5DEY4gu0iOTKgpUAAAAXSHboAAAAAAAAAAAAAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAAF0h26AAAAAABAAAAAACVG3aCj2ynsTVrlla+vu+KfhQ+sVBOGlBrHWU4WbfcAAAABgAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bf/////////8AAAABAAAAAJr/pJ5UeiUdlalnlNJP+BYL2f7UleQxGOILtIjkyoKVAAAABgAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bf/////////8AAAABAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAAAQAAAAAAlRt2go9sp7E1a5ZWvr7vin4UPrFQThpQax1lOFm33AAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAJGE5yoAAAAAABAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAAAQAAAACa/6SeVHolHZWpZ5TST/gWC9n+1JXkMRjiC7SI5MqClQAAAAFURVNUAAAAANpaWLojuOtfC0cmMh+DvQTfPDrkfXhblQTdFXrGYc0bAAAJGE5yoAAAAAAAAAAAAAAAAABKBB+2UBMP/abwcm/M1TXO+/JQWhPwkalgqizKmXyRIQx7qh6aAFYAAAAAAAAAAARW/AX3AAAAQDVB8fT2ZXF0PZqtZX9brK0kz+P4G8VKs1DkDklP6ULsvXRexXFBdH4xG8xRAsR1HJeEBH278hiBNNvUwNw6zgzGYc0bAAAAQLgZUU/oYGL7frWDQhJHhCQu9JmfqN03PrJq4/cJrN1OSUWXnmLc94sv8m2L+cxl2p0skr2Jxy+vt1Lcxkv7wAI4WbfcAAAAQHvZEVqlygIProf3jVTZohDWm2WUNrFAFXf1LctTqDCQBHph14Eo+APwrTURLLYTIvNoXeGzBKbL03SsOARWcQLkyoKVAAAAQHAvKv2/Ro4+cNh6bKQO/G9NNiUozYysGwG1GvJQkFjwy/OTsL6WBfuI0Oye84lVBVrQVk2EY1ERFhgdMpuFSg4=",
        "result_xdr": "AAAAAAAABEwAAAAAAAAACwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGAAAAAAAAAAAAAAAGAAAAAAAAAAAAAAABAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        "result_meta_xdr": "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",
        "fee_meta_xdr": "AAAAAgAAAAMAAAABAAAAAAAAAABi/B0L0JGythwN1lY0aypo19NHxvLCyO5tBEcCVvwF9w3gtrOnZAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAEAAACwAAAAAAAAAABi/B0L0JGythwN1lY0aypo19NHxvLCyO5tBEcCVvwF9w3gtrOnY/u0AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAA==",
        "memo_type": "none",
        "signatures": [
          "NUHx9PZlcXQ9mq1lf1usrSTP4/gbxUqzUOQOSU/pQuy9dF7FcUF0fjEbzFECxHUcl4QEfbvyGIE029TA3DrODA==",
          "uBlRT+hgYvt+tYNCEkeEJC70mZ+o3Tc+smrj9wms3U5JRZeeYtz3iy/ybYv5zGXanSySvYnHL6+3UtzGS/vAAg==",
          "e9kRWqXKAg+uh/eNVNmiENabZZQ2sUAVd/Uty1OoMJAEemHXgSj4A/CtNREsthMi82hd4bMEpsvTdKw4BFZxAg==",
          "cC8q/b9Gjj5w2HpspA78b002JSjNjKwbAbUa8lCQWPDL85OwvpYF+4jQ7J7ziVUFWtBWTYRjUREWGB0ym4VKDg=="
        ],
        "valid_after": "1970-01-01T00:00:00Z"
      }
    ]
  }
}"#;

	mockall::mock! {
	SpacewalkPallet {}

	#[async_trait]
	pub trait SpacewalkPallet {
	async fn report_stellar_transaction(&self, tx_envelope_xdr: &Vec<u8>) -> Result<(), runtime::Error>;
	async fn redeem(
		&self,
		asset_code: &Vec<u8>,
		asset_issuer: &Vec<u8>,
		amount: u128,
		stellar_vault_pubkey: [u8; 32],
	) -> Result<(), runtime::Error>;
	}

	}

	mockall::mock! {
	HttpClient{}

	#[async_trait]
	pub trait HorizonClient {
	async fn get_transactions(&self, url: &str) -> Result<HorizonTransactionsResponse, Error>;
	}
	}

	impl Clone for MockSpacewalkPallet {
		fn clone(&self) -> Self {
			Self::default()
		}
	}

	impl Clone for MockHttpClient {
		fn clone(&self) -> Self {
			Self::default()
		}
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn test_deposit() {
		// 1st test
		let mut client = MockHttpClient::default();
		let first_transaction: horizon::HorizonTransactionsResponse =
			serde_json::from_str(TX_STRING_1).unwrap();
		// make client return the first transaction
		client
			.expect_get_transactions()
			.times(1)
			.return_once(move |_| Ok(first_transaction));

		let mut parachain_rpc = MockSpacewalkPallet::default();
		// expect that the first transaction will not be reported
		parachain_rpc.expect_report_stellar_transaction().times(0);

		let mut fetcher =
			HorizonFetcher::new(parachain_rpc, client, STELLAR_VAULT_SECRET_KEY.to_string());

		fetcher.fetch_horizon_and_process_new_transactions().await;

		// 2nd test
		let mut client = MockHttpClient::default();
		let second_transaction = serde_json::from_str(TX_STRING_2).unwrap();
		// make client return a different transaction the next time the fetch happens
		client
			.expect_get_transactions()
			.times(1)
			.return_once(move |_| Ok(second_transaction));

		let mut parachain_rpc = MockSpacewalkPallet::default();
		// expect that the second transaction will be reported
		parachain_rpc.expect_report_stellar_transaction().times(1).returning(|_| Ok(()));

		fetcher.parachain_rpc = parachain_rpc;
		fetcher.client = client;

		fetcher.fetch_horizon_and_process_new_transactions().await;

		// 3rd test
		let mut client = MockHttpClient::default();
		let second_transaction = serde_json::from_str(TX_STRING_2).unwrap();
		// make client return the same transaction as last time
		client
			.expect_get_transactions()
			.times(1)
			.return_once(move |_| Ok(second_transaction));

		let mut parachain_rpc = MockSpacewalkPallet::default();
		// expect that nothing will be reported since no new transaction was fetched
		parachain_rpc.expect_report_stellar_transaction().times(0);

		fetcher.parachain_rpc = parachain_rpc;
		fetcher.client = client;

		fetcher.fetch_horizon_and_process_new_transactions().await;
	}
}
