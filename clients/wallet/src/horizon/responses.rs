use crate::{
	error::Error,
	horizon::{serde::*, traits::HorizonClient},
	types::{FeeAttribute, PagingToken, StatusCode},
	Slot,
};
use parity_scale_codec::{Decode, Encode};
use primitives::{
	stellar::{
		types::{
			Memo, OperationResult, SequenceNumber, TransactionResult, TransactionResultResult,
		},
		Asset, PublicKey, TransactionEnvelope, XdrCodec,
	},
	MemoTypeExt, TextMemo,
};
use serde::{de::DeserializeOwned, Deserialize};
use std::fmt::{Debug, Formatter};

const ASSET_TYPE_NATIVE: &str = "native";
const VALUE_UNKNOWN: &str = "unknown";

const RESPONSE_FIELD_TITLE: &str = "title";
const RESPONSE_FIELD_STATUS: &str = "status";
const RESPONSE_FIELD_EXTRAS: &str = "extras";
const RESPONSE_FIELD_ENVELOPE_XDR: &str = "envelope_xdr";
const RESPONSE_FIELD_DETAIL: &str = "detail";
const RESPONSE_FIELD_RESULT_CODES: &str = "result_codes";
const RESPONSE_FIELD_TRANSACTION: &str = "transaction";
const RESPONSE_FIELD_OPERATIONS: &str = "operations";

const ERROR_RESULT_TX_MALFORMED: &str = "transaction malformed";

/// a helpful macro to return either the str equivalent or the original array of u8
macro_rules! debug_str_or_vec_u8 {
	// should be &[u8]
	($res:expr) => {
		match std::str::from_utf8($res) {
			Ok(res) => format!("{}", res),
			Err(_) => format!("{:?}", $res),
		}
	};
}

/// Interprets the response from Horizon into something easier to read.
pub(crate) async fn interpret_response<T: DeserializeOwned>(
	response: reqwest::Response,
) -> Result<T, Error> {
	if response.status().is_success() {
		return response.json::<T>().await.map_err(Error::HorizonResponseError)
	}

	let resp = response
		.json::<serde_json::Value>()
		.await
		.map_err(Error::HorizonResponseError)?;

	let status =
		StatusCode::try_from(resp[RESPONSE_FIELD_STATUS].as_u64().unwrap_or(400)).unwrap_or(400);

	let title = resp[RESPONSE_FIELD_TITLE].as_str().unwrap_or(VALUE_UNKNOWN);

	let error = match status {
		400 => {
			let envelope_xdr = resp[RESPONSE_FIELD_EXTRAS][RESPONSE_FIELD_ENVELOPE_XDR]
				.as_str()
				.unwrap_or(VALUE_UNKNOWN);

			match title.to_lowercase().as_str() {
				// this particular status does not have the "result_code",
				// so the "detail" portion will be used for "reason".
				ERROR_RESULT_TX_MALFORMED => {
					let detail = resp[RESPONSE_FIELD_DETAIL].as_str().unwrap_or(VALUE_UNKNOWN);
					Error::HorizonSubmissionError {
						title: title.to_string(),
						status,
						reason: detail.to_string(),
						result_code_op: vec![],
						envelope_xdr: Some(envelope_xdr.to_string()),
					}
				},
				_ => {
					let result_code_tx = resp[RESPONSE_FIELD_EXTRAS][RESPONSE_FIELD_RESULT_CODES]
						[RESPONSE_FIELD_TRANSACTION]
						.as_str()
						.unwrap_or(VALUE_UNKNOWN);

					let result_code_op: Vec<String> = resp[RESPONSE_FIELD_EXTRAS]
						[RESPONSE_FIELD_RESULT_CODES][RESPONSE_FIELD_OPERATIONS]
						.as_array()
						.unwrap_or(&vec![])
						.iter()
						.map(|v| v.as_str().unwrap_or(VALUE_UNKNOWN).to_string())
						.collect();

					Error::HorizonSubmissionError {
						title: title.to_string(),
						status,
						reason: result_code_tx.to_string(),
						result_code_op,
						envelope_xdr: Some(envelope_xdr.to_string()),
					}
				},
			}
		},
		_ => {
			let detail = resp[RESPONSE_FIELD_DETAIL].as_str().unwrap_or(VALUE_UNKNOWN);

			Error::HorizonSubmissionError {
				title: title.to_string(),
				status,
				reason: detail.to_string(),
				result_code_op: vec![],
				envelope_xdr: None,
			}
		},
	};

	tracing::error!("Response returned an error: {:?}", &error);
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
#[derive(Clone, Deserialize, Encode, Decode, Default)]
pub struct TransactionResponse {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub id: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_u128")]
	pub paging_token: PagingToken,
	pub successful: bool,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub hash: Vec<u8>,
	pub ledger: Slot,
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

impl Debug for TransactionResponse {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let memo = match &self.memo {
			None => "".to_string(),
			Some(memo) => {
				debug_str_or_vec_u8!(memo)
			},
		};

		f.debug_struct("TransactionResponse")
			.field("id", &debug_str_or_vec_u8!(&self.id))
			.field("paging_token", &self.paging_token)
			.field("successful", &self.successful)
			.field("hash", &debug_str_or_vec_u8!(&self.hash))
			.field("ledger", &self.ledger)
			.field("created_at", &debug_str_or_vec_u8!(&self.created_at))
			.field("source_account", &debug_str_or_vec_u8!(&self.source_account))
			.field("source_account_sequence", &debug_str_or_vec_u8!(&self.source_account_sequence))
			.field("fee_account", &debug_str_or_vec_u8!(&self.fee_account))
			.field("fee_charged", &self.fee_charged)
			.field("max_fee", &debug_str_or_vec_u8!(&self.max_fee))
			.field("operation_count", &self.operation_count)
			.field("envelope_xdr", &debug_str_or_vec_u8!(&self.envelope_xdr))
			.field("result_xdr", &debug_str_or_vec_u8!(&self.result_xdr))
			.field("result_meta_xdr", &debug_str_or_vec_u8!(&self.result_meta_xdr))
			.field("memo_type", &debug_str_or_vec_u8!(&self.memo_type))
			.field("memo", &memo)
			.finish()
	}
}

#[allow(dead_code)]
impl TransactionResponse {
	pub(crate) fn ledger(&self) -> Slot {
		self.ledger
	}

	pub fn memo_text(&self) -> Option<&TextMemo> {
		if Memo::is_type_text(&self.memo_type) {
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

	pub fn source_account(&self) -> Result<PublicKey, Error> {
		PublicKey::from_encoding(&self.source_account).map_err(|_| Error::DecodeError)
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

	pub fn transaction_hash(&self) -> String {
		debug_str_or_vec_u8!(&self.hash)
	}
}

#[derive(Deserialize)]
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

impl Debug for HorizonAccountResponse {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("HorizonAccountResponse")
			.field("id", &debug_str_or_vec_u8!(&self.id))
			.field("account_id", &debug_str_or_vec_u8!(&self.account_id))
			.field("sequence", &self.sequence)
			.field("balances", &self.balances)
			.finish()
	}
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
		if &self.asset_type == ASSET_TYPE_NATIVE.as_bytes() {
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

#[derive(Deserialize, Debug)]
pub struct FeeDistribution {
	#[serde(deserialize_with = "de_string_to_u32")]
	pub max: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub min: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub mode: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p10: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p20: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p30: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p40: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p50: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p60: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p70: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p80: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p90: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p95: u32,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub p99: u32,
}

#[derive(Deserialize, Debug)]
pub struct FeeStats {
	#[serde(deserialize_with = "de_string_to_u32")]
	pub last_ledger: Slot,
	#[serde(deserialize_with = "de_string_to_u32")]
	pub last_ledger_base_fee: u32,
	#[serde(deserialize_with = "de_string_to_f64")]
	pub ledger_capacity_usage: f64,
	pub fee_charged: FeeDistribution,
	pub max_fee: FeeDistribution,
}

impl FeeStats {
	pub fn fee_charged_by(&self, fee_attr: FeeAttribute) -> u32 {
		match fee_attr {
			FeeAttribute::max => self.fee_charged.max,
			FeeAttribute::min => self.fee_charged.min,
			FeeAttribute::mode => self.fee_charged.mode,
			FeeAttribute::p10 => self.fee_charged.p10,
			FeeAttribute::p20 => self.fee_charged.p20,
			FeeAttribute::p30 => self.fee_charged.p30,
			FeeAttribute::p40 => self.fee_charged.p40,
			FeeAttribute::p50 => self.fee_charged.p50,
			FeeAttribute::p60 => self.fee_charged.p60,
			FeeAttribute::p70 => self.fee_charged.p70,
			FeeAttribute::p80 => self.fee_charged.p80,
			FeeAttribute::p90 => self.fee_charged.p90,
			FeeAttribute::p95 => self.fee_charged.p95,
			FeeAttribute::p99 => self.fee_charged.p99,
		}
	}
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
	pub last_modified_ledger: Slot,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub last_modified_time: Vec<u8>,
}

// This represents a Claimant
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct Claimant {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub destination: Vec<u8>,
	// For now we assume that the predicate is always unconditional
	pub predicate: ClaimantPredicate,
}

#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct ClaimantPredicate {
	pub unconditional: Option<bool>,
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

	/// returns the next TransactionResponse
	pub async fn next(&mut self) -> Option<TransactionResponse> {
		match self.get_top_record() {
			Some(record) => Some(record),
			None => {
				let _ = self.jump_to_next_page().await?;
				self.get_top_record()
			},
		}
	}

	/// returns the last TransactionResponse in the list
	pub fn next_back(&mut self) -> Option<TransactionResponse> {
		self.records.pop()
	}

	pub fn get_middle(&mut self) -> Option<TransactionResponse> {
		if !self.is_empty() {
			let idx = self.records.len() / 2;
			return Some(self.records.remove(idx))
		}
		None
	}

	pub fn remove_first_half_records(&mut self) {
		let idx = self.records.len() / 2;
		self.records = self.records[..idx].to_vec();
	}

	pub fn remove_last_half_records(&mut self) {
		let idx = self.records.len() / 2;
		self.records = self.records[idx..].to_vec();
	}

	pub async fn jump_to_next_page(&mut self) -> Option<()>  {
		let response: HorizonTransactionsResponse =
			self.client.get_from_url(&self.next_page).await.ok()?;
		self.next_page = response.next_page();
		self.records = response.records();

		Some(())

	}
}
