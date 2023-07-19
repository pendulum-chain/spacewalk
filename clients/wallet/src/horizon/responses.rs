use crate::{
	error::Error,
	horizon::{serde::*, traits::HorizonClient, Ledger},
	types::PagingToken,
};
use parity_scale_codec::{Decode, Encode};
use primitives::{
	stellar::{
		types::{OperationResult, SequenceNumber, TransactionResult, TransactionResultResult},
		Asset, TransactionEnvelope, XdrCodec,
	},
	TextMemo,
};
use serde::{de::DeserializeOwned, Deserialize};

/// Interprets the response from Horizon into something easier to read.
pub(crate) async fn interpret_response<T: DeserializeOwned>(
	response: reqwest::Response,
) -> Result<T, Error> {
	if response.status().is_success() {
		return response.json::<T>().await.map_err(Error::HttpFetchingError)
	}

	let resp = response.json::<serde_json::Value>().await.map_err(Error::HttpFetchingError)?;

	let unknown = "unknown";
	let title = resp["title"].as_str().unwrap_or(unknown);
	let status = u16::try_from(resp["status"].as_u64().unwrap_or(400)).unwrap_or(400);

	let error = match status {
		400 => {
			let envelope_xdr = resp["extras"]["envelope_xdr"].as_str().unwrap_or(unknown);

			match title.to_lowercase().as_str() {
				// this particular status does not have the "result_code",
				// so the "detail" portion will be used for "reason".
				"transaction malformed" => {
					let detail = resp["detail"].as_str().unwrap_or(unknown);

					Error::HorizonSubmissionError {
						title: title.to_string(),
						status,
						reason: detail.to_string(),
						envelope_xdr: Some(envelope_xdr.to_string()),
					}
				},
				_ => {
					let result_code =
						resp["extras"]["result_codes"]["transaction"].as_str().unwrap_or(unknown);

					Error::HorizonSubmissionError {
						title: title.to_string(),
						status,
						reason: result_code.to_string(),
						envelope_xdr: Some(envelope_xdr.to_string()),
					}
				},
			}
		},
		_ => {
			let detail = resp["detail"].as_str().unwrap_or(unknown);

			Error::HorizonSubmissionError {
				title: title.to_string(),
				status,
				reason: detail.to_string(),
				envelope_xdr: None,
			}
		},
	};

	tracing::error!("Response returned error: {:?}", &error);
	Err(error)
}

// The following structs represent the whole response when fetching any Horizon API
// In this particular case we assume the embedded payload will allways be for transactions
// ref https://developers.stellar.org/api/introduction/response-format/
#[derive(Deserialize, Debug)]
pub struct HorizonTransactionsResponse {
	pub _embedded: EmbeddedTransactions,
	_links: HorizonLinks,
}

#[derive(Deserialize, Debug)]
pub struct HorizonLinks {
	next: HrefPage,
	prev: HrefPage,
}

#[derive(Deserialize, Debug)]
pub struct HrefPage {
	pub href: String,
}

#[allow(dead_code)]
impl HorizonTransactionsResponse {
	fn previous_page(&self) -> String {
		self._links.prev.href.clone()
	}

	pub(crate) fn next_page(&self) -> String {
		self._links.next.href.clone()
	}

	pub(crate) fn records(self) -> Vec<TransactionResponse> {
		self._embedded.records
	}
}

#[derive(Deserialize, Debug)]
pub struct EmbeddedTransactions {
	pub records: Vec<TransactionResponse>,
}

// This represents each record for a transaction in the Horizon API response
#[derive(Clone, Deserialize, Encode, Decode, Default, Debug)]
pub struct TransactionResponse {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_u128")]
	pub paging_token: PagingToken,
	pub successful: bool,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub hash: Vec<u8>,
	pub ledger: Ledger,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub created_at: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub source_account: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub source_account_sequence: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub fee_account: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_u64")]
	pub fee_charged: u64,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub max_fee: Vec<u8>,
	operation_count: u32,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub envelope_xdr: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub result_xdr: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub result_meta_xdr: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub fee_meta_xdr: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub memo_type: Vec<u8>,
	#[serde(default)]
	#[serde(deserialize_with = "de_string_to_optional_bytes")]
	pub memo: Option<Vec<u8>>,
}

#[allow(dead_code)]
impl TransactionResponse {
	pub(crate) fn ledger(&self) -> Ledger {
		self.ledger
	}

	pub fn memo_text(&self) -> Option<&TextMemo> {
		if self.memo_type == b"text" {
			self.memo.as_ref()
		} else {
			None
		}
	}

	pub fn to_envelope(&self) -> Result<TransactionEnvelope, Error> {
		TransactionEnvelope::from_base64_xdr(self.envelope_xdr.clone())
			.map_err(|_| Error::DecodeError)
	}

	pub fn source_account_sequence(&self) -> Result<SequenceNumber, Error> {
		let res = String::from_utf8(self.source_account_sequence.clone())
			.map_err(|_| Error::DecodeError)?;

		res.parse::<SequenceNumber>().map_err(|_| Error::DecodeError)
	}

	pub fn get_successful_operations_result(&self) -> Result<Vec<OperationResult>, Error> {
		if let TransactionResultResult::TxSuccess(res) =
			TransactionResult::from_base64_xdr(&self.result_xdr)
				.map_err(|_| Error::DecodeError)?
				.result
		{
			return Ok(res.get_vec().to_vec())
		}

		Ok(vec![])
	}
}

#[derive(Deserialize, Debug)]
pub struct HorizonAccountResponse {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub account_id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_i64")]
	pub sequence: i64,
	pub balances: Vec<HorizonBalance>,
	// ...
}

impl HorizonAccountResponse {
	pub fn is_trustline_exist(&self, asset: &Asset) -> bool {
		for balance in &self.balances {
			if let Some(balance_asset) = balance.get_asset() {
				if &balance_asset == asset {
					return true
				}
			}
		}

		false
	}
}

#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct HorizonBalance {
	#[serde(deserialize_with = "de_string_to_f64")]
	pub balance: f64,
	#[serde(default)]
	#[serde(deserialize_with = "de_string_to_optional_bytes")]
	pub asset_code: Option<Vec<u8>>,
	#[serde(default)]
	#[serde(deserialize_with = "de_string_to_optional_bytes")]
	pub asset_issuer: Option<Vec<u8>>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub asset_type: Vec<u8>,
}

impl HorizonBalance {
	/// returns what kind of asset the Balance is
	pub fn get_asset(&self) -> Option<Asset> {
		if &self.asset_type == "native".as_bytes() {
			return Some(Asset::AssetTypeNative)
		}

		match Asset::from_asset_code(&self.asset_code.clone()?, &self.asset_issuer.clone()?) {
			Ok(asset) => Some(asset),
			Err(e) => {
				tracing::warn!("failed to convert to asset: {e:?}");
				None
			},
		}
	}
}

// The following structs represent the whole response when fetching any Horizon API
// for retreiving a list of claimable balances for an account
#[derive(Deserialize, Debug)]
pub struct EmbeddedClaimableBalance {
	pub records: Vec<ClaimableBalance>,
}

#[derive(Deserialize, Debug)]
pub struct HorizonClaimableBalanceResponse {
	#[serde(flatten)]
	pub claimable_balance: ClaimableBalance,
}

// This represents each record for a claimable balance in the Horizon API response
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct ClaimableBalance {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub paging_token: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub asset: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub amount: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub sponsor: Vec<u8>,

	pub claimants: Vec<Claimant>,
	pub last_modified_ledger: Ledger,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub last_modified_time: Vec<u8>,
}

// This represents a Claimant
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct Claimant {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub destination: Vec<u8>,
	// For now we assume that the predicate is always unconditional
	// pub predicate: serde_json::Value,
}

/// An iter structure equivalent to a list of TransactionResponse
pub struct TransactionsResponseIter<C> {
	/// holds a maximum of 200 items
	pub(crate) records: Vec<TransactionResponse>,
	/// the url for the next page
	pub(crate) next_page: String,
	/// a client capable to do GET operation
	pub(crate) client: C,
}

impl<C: HorizonClient> TransactionsResponseIter<C> {
	pub(crate) fn is_empty(&self) -> bool {
		self.records.is_empty()
	}

	#[doc(hidden)]
	// returns the first record of the list
	fn get_top_record(&mut self) -> Option<TransactionResponse> {
		if !self.is_empty() {
			return Some(self.records.remove(0))
		}
		None
	}

	/// returns the next TransactionResponse in the list
	pub async fn next(&mut self) -> Option<TransactionResponse> {
		match self.get_top_record() {
			Some(record) => Some(record),
			None => {
				// call the next page
				tracing::trace!("calling next page: {}", &self.next_page);

				let response: HorizonTransactionsResponse =
					self.client.get_from_url(&self.next_page).await.ok()?;
				self.next_page = response.next_page();
				self.records = response.records();

				self.get_top_record()
			},
		}
	}
}
