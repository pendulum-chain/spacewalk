use std::{collections::HashMap, convert::TryInto};

use crate::{
	system::{VaultData, VaultIdManager},
	DecimalsLookupImpl, Error,
};
use async_trait::async_trait;
use lazy_static::lazy_static;
use primitives::{stellar, Asset, DecimalsLookup};
use runtime::{
	prometheus::{
		gather, proto::MetricFamily, Encoder, Gauge, GaugeVec, IntCounter, IntGaugeVec, Opts,
		Registry, TextEncoder,
	},
	types::currency_id::CurrencyIdExt,
	AggregateUpdatedEvent, CollateralBalancesPallet, CurrencyId, Error as RuntimeError, FixedU128,
	IssuePallet, IssueRequestStatus, OracleKey, RedeemPallet, RedeemRequestStatus, SecurityPallet,
	SpacewalkParachain, SpacewalkRedeemRequest, UtilFuncs, VaultId, VaultRegistryPallet, H256,
};
use service::{
	warp::{Rejection, Reply},
	Error as ServiceError,
};
use std::time::Duration;
use tokio::time::sleep;
use tokio_metrics::TaskMetrics;
use wallet::HorizonBalance;

const SLEEP_DURATION: Duration = Duration::from_secs(5 * 60);

const CURRENCY_LABEL: &str = "currency";
const XLM_BALANCE_TYPE_LABEL: &str = "type";
const REQUEST_STATUS_LABEL: &str = "status";
const TASK_NAME: &str = "task";
const TOKIO_POLLING_INTERVAL_MS: u64 = 10000;
const DISPLAY_NAME_LABEL: &str = "display_name";

// Metrics are stored under the [`CURRENCY_LABEL`] key so that multiple vaults can be easily
// monitored at the same time.
lazy_static! {
	pub static ref REGISTRY: Registry = Registry::new();
	pub static ref AVERAGE_XLM_FEE: GaugeVec =
		GaugeVec::new(Opts::new("avg_stellar_fee", "Average Stellar Fee"), &[CURRENCY_LABEL])
			.expect("Failed to create prometheus metric");
	pub static ref LOCKED_COLLATERAL: GaugeVec =
		GaugeVec::new(Opts::new("locked_collateral", "Locked Collateral"), &[CURRENCY_LABEL])
			.expect("Failed to create prometheus metric");
	pub static ref COLLATERALIZATION: GaugeVec =
		GaugeVec::new(Opts::new("collateralization", "Collateralization"), &[CURRENCY_LABEL])
			.expect("Failed to create prometheus metric");
	pub static ref REQUIRED_COLLATERAL: GaugeVec =
		GaugeVec::new(Opts::new("required_collateral", "Required Collateral"), &[CURRENCY_LABEL])
			.expect("Failed to create prometheus metric");
	pub static ref MEAN_IDLE_DURATION: IntGaugeVec =
		IntGaugeVec::new(Opts::new("mean_idle_duration_ms", "Total Idle Duration"), &[TASK_NAME])
			.expect("Failed to create prometheus metric");
	pub static ref MEAN_POLL_DURATION: IntGaugeVec =
		IntGaugeVec::new(Opts::new("mean_poll_duration_ms", "Total Poll Duration"), &[TASK_NAME])
			.expect("Failed to create prometheus metric");
	pub static ref MEAN_SCHEDULED_DURATION: IntGaugeVec = IntGaugeVec::new(
		Opts::new("mean_scheduled_duration_ms", "Total Scheduled Duration"),
		&[TASK_NAME]
	)
	.expect("Failed to create prometheus metric");
	pub static ref XLM_BALANCE: GaugeVec = GaugeVec::new(
		Opts::new("stellar_balance", "Stellar Balance"),
		&[CURRENCY_LABEL, XLM_BALANCE_TYPE_LABEL, DISPLAY_NAME_LABEL]
	)
	.expect("Failed to create prometheus metric");
	pub static ref ISSUES: GaugeVec = GaugeVec::new(
		Opts::new("issue_count", "Number of issues"),
		&[CURRENCY_LABEL, REQUEST_STATUS_LABEL, DISPLAY_NAME_LABEL]
	)
	.expect("Failed to create prometheus metric");
	pub static ref REDEEMS: GaugeVec = GaugeVec::new(
		Opts::new("redeem_count", "Number of redeems"),
		&[CURRENCY_LABEL, REQUEST_STATUS_LABEL, DISPLAY_NAME_LABEL]
	)
	.expect("Failed to create prometheus metric");
	pub static ref NATIVE_CURRENCY_BALANCE: Gauge =
		Gauge::new("native_currency_balance", "Native Currency Balance")
			.expect("Failed to create prometheus metric");
	pub static ref RESTART_COUNT: IntCounter =
		IntCounter::new("restart_count", "Number of service restarts")
			.expect("Failed to create prometheus metric");
}
const STELLAR_NATIVE_ASSET_TYPE: [u8; 6] = *b"native";

#[derive(Clone, Debug)]
struct XLMBalance {
	upperbound: Gauge,
	lowerbound: Gauge,
	actual: Gauge,
}

#[derive(Clone, Debug)]
struct RequestCounter {
	open_count: Gauge,
	completed_count: Gauge,
	expired_count: Gauge,
}

#[derive(Clone, Debug)]
pub struct PerCurrencyMetrics {
	locked_collateral: Gauge,
	collateralization: Gauge,
	required_collateral: Gauge,
	asset_balance: XLMBalance,
	issues: RequestCounter,
	redeems: RequestCounter,
}

#[async_trait]
pub trait VaultDataReader {
	async fn get_entries(&self) -> Vec<VaultData>;
}

#[async_trait]
impl VaultDataReader for VaultIdManager {
	async fn get_entries(&self) -> Vec<VaultData> {
		self.get_entries().await
	}
}

struct DisplayLabels {
	upperbound_label: String,
	lowerbound_label: String,
	actual_label: String,
	open_label: String,
	completed_label: String,
	expired_label: String,
}

impl PerCurrencyMetrics {
	pub fn new(vault_id: &VaultId) -> Self {
		let label = format!(
			"{}_{}",
			vault_id.collateral_currency().inner().unwrap_or_default(),
			vault_id.wrapped_currency().inner().unwrap_or_default()
		);

		let display_label = format!(
			"{}_{}",
			Self::format_currency_for_display(vault_id.collateral_currency()),
			Self::format_currency_for_display(vault_id.wrapped_currency())
		);
		Self::new_with_label(label.as_ref(), display_label.as_ref())
	}

	// construct a dummy metrics struct for testing purposes
	pub fn dummy() -> Self {
		Self::new_with_label("dummy", "dummy")
	}

	fn format_currency_for_display(currency: CurrencyId) -> String {
		match currency {
			CurrencyId::Stellar(asset) => match asset {
				Asset::AlphaNum4 { code, .. } =>
					String::from_utf8(code.to_vec()).unwrap_or_default().replace('\"', ""),
				Asset::AlphaNum12 { code, .. } =>
					String::from_utf8(code.to_vec()).unwrap_or_default().replace('\"', ""),
				Asset::StellarNative => "XLM".to_owned(),
			},
			CurrencyId::Native => "Native".to_owned(),
			CurrencyId::ZenlinkLPToken(token1_id, token1_type, token2_id, token2_type) => {
				format!("LP_{}_{}_{}_{}", token1_id, token1_type, token2_id, token2_type)
			},
			CurrencyId::XCM(_) => currency.inner().unwrap_or_default(),
			_ => "Unknown".to_owned(),
		}
	}

	fn new_with_label(label: &str, display_label: &str) -> Self {
		let labels = HashMap::from([(CURRENCY_LABEL, label)]);

		let labels_struct = DisplayLabels {
			upperbound_label: format!("{} - required_upperbound", display_label),
			lowerbound_label: format!("{} - required_lowerbound", display_label),
			actual_label: format!("{} - actual", display_label),
			open_label: format!("{} - open", display_label),
			completed_label: format!("{} - completed", display_label),
			expired_label: format!("{} - expired", display_label),
		};

		let stellar_balance_gauge = |balance_type: &'static str| {
			let display_name = match balance_type {
				"required_upperbound" => &labels_struct.upperbound_label,
				"required_lowerbound" => &labels_struct.lowerbound_label,
				"actual" => &labels_struct.actual_label,
				_ => "",
			};

			let labels = HashMap::<&str, &str>::from([
				(CURRENCY_LABEL, label),
				(XLM_BALANCE_TYPE_LABEL, balance_type),
				(DISPLAY_NAME_LABEL, display_name),
			]);
			XLM_BALANCE.with(&labels)
		};
		let request_type_label = |request_type: &'static str| {
			let display_name = match request_type {
				"open" => &labels_struct.open_label,
				"completed" => &labels_struct.completed_label,
				"expired" => &labels_struct.expired_label,
				_ => "",
			};

			HashMap::<&str, &str>::from([
				(CURRENCY_LABEL, label),
				(REQUEST_STATUS_LABEL, request_type),
				(DISPLAY_NAME_LABEL, display_name),
			])
		};

		Self {
			locked_collateral: LOCKED_COLLATERAL.with(&labels),
			collateralization: COLLATERALIZATION.with(&labels),
			required_collateral: REQUIRED_COLLATERAL.with(&labels),
			asset_balance: XLMBalance {
				upperbound: stellar_balance_gauge("required_upperbound"),
				lowerbound: stellar_balance_gauge("required_lowerbound"),
				actual: stellar_balance_gauge("actual"),
			},
			issues: RequestCounter {
				open_count: ISSUES.with(&request_type_label("open")),
				completed_count: ISSUES.with(&request_type_label("completed")),
				expired_count: ISSUES.with(&request_type_label("expired")),
			},
			redeems: RequestCounter {
				open_count: REDEEMS.with(&request_type_label("open")),
				completed_count: REDEEMS.with(&request_type_label("completed")),
				expired_count: REDEEMS.with(&request_type_label("expired")),
			},
		}
	}

	pub async fn initialize_values(parachain_rpc: SpacewalkParachain, vault: &VaultData) {
		publish_stellar_balance(vault).await;

		let _ = tokio::join!(
			publish_expected_stellar_balance(vault, &parachain_rpc),
			publish_locked_collateral(vault, parachain_rpc.clone()),
			publish_required_collateral(vault, parachain_rpc.clone()),
			publish_collateralization(vault, parachain_rpc.clone()),
		);
	}
}

pub fn register_custom_metrics() -> Result<(), RuntimeError> {
	REGISTRY.register(Box::new(AVERAGE_XLM_FEE.clone()))?;
	REGISTRY.register(Box::new(LOCKED_COLLATERAL.clone()))?;
	REGISTRY.register(Box::new(COLLATERALIZATION.clone()))?;
	REGISTRY.register(Box::new(REQUIRED_COLLATERAL.clone()))?;
	REGISTRY.register(Box::new(XLM_BALANCE.clone()))?;
	REGISTRY.register(Box::new(NATIVE_CURRENCY_BALANCE.clone()))?;
	REGISTRY.register(Box::new(ISSUES.clone()))?;
	REGISTRY.register(Box::new(REDEEMS.clone()))?;
	REGISTRY.register(Box::new(MEAN_IDLE_DURATION.clone()))?;
	REGISTRY.register(Box::new(MEAN_POLL_DURATION.clone()))?;
	REGISTRY.register(Box::new(MEAN_SCHEDULED_DURATION.clone()))?;
	REGISTRY.register(Box::new(RESTART_COUNT.clone()))?;

	Ok(())
}

fn serialize(metrics: &[MetricFamily]) -> String {
	let encoder = TextEncoder::new();
	let mut buffer = Vec::new();
	if let Err(e) = encoder.encode(metrics, &mut buffer) {
		tracing::error!("Could not encode metrics: {}", e);
	};
	let res = match String::from_utf8(buffer.clone()) {
		Ok(v) => v,
		Err(e) => {
			tracing::error!("Metrics could not be parsed `from_utf8`: {}", e);
			String::default()
		},
	};
	buffer.clear();
	res
}

pub async fn metrics_handler() -> Result<impl Reply, Rejection> {
	let mut metrics = serialize(&REGISTRY.gather());
	let custom_metrics = serialize(&gather());
	metrics.push_str(&custom_metrics);
	Ok(metrics)
}

fn raw_value_as_currency(value: u128, currency: CurrencyId) -> Result<f64, ServiceError<Error>> {
	let scaling_factor = DecimalsLookupImpl::one(currency) as f64;
	Ok(value as f64 / scaling_factor)
}

pub async fn publish_locked_collateral<P: VaultRegistryPallet>(
	vault: &VaultData,
	parachain_rpc: P,
) -> Result<(), ServiceError<Error>> {
	if let Ok(actual_collateral) =
		parachain_rpc.get_vault_total_collateral(vault.vault_id.clone()).await
	{
		let actual_collateral =
			raw_value_as_currency(actual_collateral, vault.vault_id.collateral_currency())?;
		vault.metrics.locked_collateral.set(actual_collateral);
	}
	Ok(())
}

pub async fn publish_required_collateral<P: VaultRegistryPallet>(
	vault: &VaultData,
	parachain_rpc: P,
) -> Result<(), ServiceError<Error>> {
	if let Ok(required_collateral) =
		parachain_rpc.get_required_collateral_for_vault(vault.vault_id.clone()).await
	{
		let required_collateral =
			raw_value_as_currency(required_collateral, vault.vault_id.collateral_currency())?;
		vault.metrics.required_collateral.set(required_collateral);
	}
	Ok(())
}

pub async fn publish_collateralization<P: VaultRegistryPallet>(
	vault: &VaultData,
	parachain_rpc: P,
) {
	// if the collateralization is infinite, return 0 rather than logging an error, so
	// the metrics do change in case of a replacement
	let result = parachain_rpc
		.get_collateralization_from_vault(vault.vault_id.clone(), false)
		.await;

	let collateralization = result.unwrap_or(0);
	let float_collateralization_percentage = FixedU128::from_inner(collateralization).to_float();
	vault.metrics.collateralization.set(float_collateralization_percentage);
}

pub async fn update_stellar_metrics<P: VaultRegistryPallet>(vault: &VaultData, parachain_rpc: &P) {
	publish_stellar_balance(vault).await;
	let _ = publish_expected_stellar_balance(vault, parachain_rpc).await;
}

async fn publish_stellar_balance(vault: &VaultData) {
	match vault.stellar_wallet.read().await.get_balances().await {
		Ok(balance) => {
			let currency_id = vault.vault_id.wrapped_currency();
			let asset: Result<stellar::Asset, _> = currency_id.try_into();
			let actual_balance = match asset {
				Ok(asset) => get_balances_for_asset(asset, balance).unwrap_or(0_f64),
				Err(e) => {
					// unexpected error, but not critical so just continue
					tracing::warn!("Failed to get balance: {}", e);
					0_f64
				},
			};
			vault.metrics.asset_balance.actual.set(actual_balance);
		},
		Err(e) => {
			// unexpected error, but not critical so just continue
			tracing::warn!("Failed to get balance: {}", e);
			vault.metrics.asset_balance.actual.set(0 as f64);
		},
	}
}

fn get_balances_for_asset(asset: stellar::Asset, balances: Vec<HorizonBalance>) -> Option<f64> {
	let asset_balance: Option<f64> = match asset {
		stellar::Asset::AssetTypeNative => balances
			.iter()
			.find(|i| i.asset_type == STELLAR_NATIVE_ASSET_TYPE.to_vec())
			.map(|i| i.balance),
		stellar::Asset::AssetTypeCreditAlphanum4(a4) => balances
			.iter()
			.find(|i| {
				i.asset_issuer.clone().unwrap_or_default() == a4.issuer.to_encoding() &&
					i.asset_code.clone().unwrap_or_default() == a4.asset_code.to_vec()
			})
			.map(|i| i.balance),
		stellar::Asset::AssetTypeCreditAlphanum12(a12) => balances
			.iter()
			.find(|i| {
				i.asset_issuer.clone().unwrap_or_default() == a12.issuer.to_encoding() &&
					i.asset_code.clone().unwrap_or_default() == a12.asset_code.to_vec()
			})
			.map(|i| i.balance),
		_ => {
			tracing::warn!("Unsupported stellar asset type");
			None
		},
	};
	asset_balance
}

async fn publish_native_currency_balance<P: CollateralBalancesPallet + UtilFuncs>(
	parachain_rpc: &P,
) -> Result<(), ServiceError<Error>> {
	let native_currency = parachain_rpc.get_native_currency_id();
	let account_id = parachain_rpc.get_account_id();
	if let Ok(balance) = parachain_rpc.get_native_balance_for_id(account_id).await {
		let balance = raw_value_as_currency(balance, native_currency)?;
		NATIVE_CURRENCY_BALANCE.set(balance);
	}
	Ok(())
}

pub fn increment_restart_counter() {
	RESTART_COUNT.inc();
}

async fn publish_issue_count<V: VaultDataReader, P: IssuePallet + UtilFuncs>(
	parachain_rpc: &P,
	vault_id_manager: &V,
) {
	if let Ok(issues) = parachain_rpc
		.get_vault_issue_requests(parachain_rpc.get_account_id().clone())
		.await
	{
		for vault in vault_id_manager.get_entries().await {
			let relevant_issues: Vec<_> = issues
				.iter()
				.filter(|(_, issue)| issue.vault == vault.vault_id)
				.map(|(_, issue)| issue.status.clone())
				.collect();

			vault.metrics.issues.open_count.set(
				relevant_issues
					.iter()
					.filter(|status| matches!(status, IssueRequestStatus::Pending))
					.count() as f64,
			);
			vault.metrics.issues.completed_count.set(
				relevant_issues
					.iter()
					.filter(|status| matches!(status, IssueRequestStatus::Completed))
					.count() as f64,
			);
			vault.metrics.issues.expired_count.set(
				relevant_issues
					.iter()
					.filter(|status| matches!(status, IssueRequestStatus::Cancelled))
					.count() as f64,
			);
		}
	}
}

async fn publish_redeem_count<V: VaultDataReader>(
	vault_id_manager: &V,
	redeems: &[(H256, runtime::SpacewalkRedeemRequest)],
) {
	for vault in vault_id_manager.get_entries().await {
		let relevant_redeems: Vec<_> = redeems
			.iter()
			.filter(|(_, redeem)| redeem.vault == vault.vault_id)
			.map(|(_, redeem)| redeem.status.clone())
			.collect();

		vault.metrics.redeems.open_count.set(
			relevant_redeems
				.iter()
				.filter(|status| matches!(status, RedeemRequestStatus::Pending))
				.count() as f64,
		);
		vault.metrics.redeems.completed_count.set(
			relevant_redeems
				.iter()
				.filter(|status| matches!(status, RedeemRequestStatus::Completed))
				.count() as f64,
		);
		vault.metrics.redeems.expired_count.set(
			relevant_redeems
				.iter()
				.filter(|status| {
					matches!(
						status,
						RedeemRequestStatus::Reimbursed(_) | RedeemRequestStatus::Retried
					)
				})
				.count() as f64,
		);
	}
}

pub async fn monitor_bridge_metrics(
	parachain_rpc: SpacewalkParachain,
	vault_id_manager: VaultIdManager,
) -> Result<(), ServiceError<Error>> {
	let parachain_rpc = &parachain_rpc;
	let vault_id_manager = &vault_id_manager;
	parachain_rpc
		.on_event::<AggregateUpdatedEvent, _, _, _>(
			|event| async move {
				let updated_currencies = event.values.iter().map(|(key, _value)| match key {
					OracleKey::ExchangeRate(currency_id) => currency_id,
				});
				let vaults = vault_id_manager.get_entries().await;
				for currency_id in updated_currencies {
					for vault in vaults
						.iter()
						.filter(|vault| vault.vault_id.collateral_currency() == **currency_id)
					{
						let _ = publish_locked_collateral(vault, parachain_rpc.clone()).await;
						let _ = publish_required_collateral(vault, parachain_rpc.clone()).await;
						publish_collateralization(vault, parachain_rpc.clone()).await;
					}
				}
			},
			|error| tracing::error!("Error reading SetExchangeRate event: {}", error.to_string()),
		)
		.await?;
	Ok(())
}

pub async fn poll_metrics<
	P: CollateralBalancesPallet
		+ RedeemPallet
		+ IssuePallet
		+ SecurityPallet
		+ VaultRegistryPallet
		+ UtilFuncs,
>(
	parachain_rpc: P,
	vault_id_manager: VaultIdManager,
) -> Result<(), ServiceError<Error>> {
	let parachain_rpc = &parachain_rpc;
	let vault_id_manager = &vault_id_manager;

	loop {
		publish_native_currency_balance(parachain_rpc).await?;
		publish_issue_count(parachain_rpc, vault_id_manager).await;

		let pass_all_filter = |item: (H256, SpacewalkRedeemRequest)| Some(item);

		if let Ok(redeems) = parachain_rpc
			.get_vault_redeem_requests::<(H256, SpacewalkRedeemRequest)>(
				parachain_rpc.get_account_id().clone(),
				Box::new(pass_all_filter),
			)
			.await
		{
			publish_redeem_count(vault_id_manager, &redeems).await;
		}

		for vault in vault_id_manager.get_entries().await {
			update_stellar_metrics(&vault, parachain_rpc).await;
		}

		sleep(SLEEP_DURATION).await;
	}
}

pub async fn publish_expected_stellar_balance<P: VaultRegistryPallet>(
	vault: &VaultData,
	parachain_rpc: &P,
) -> Result<(), ServiceError<Error>> {
	if let Ok(v) = parachain_rpc.get_vault(&vault.vault_id).await {
		let lowerbound = v.issued_tokens.saturating_sub(v.to_be_redeemed_tokens);
		let upperbound = v.issued_tokens.saturating_add(v.to_be_issued_tokens);
		let scaling_factor = DecimalsLookupImpl::one(vault.vault_id.wrapped_currency()) as f64;

		vault.metrics.asset_balance.lowerbound.set(lowerbound as f64 / scaling_factor);
		vault.metrics.asset_balance.upperbound.set(upperbound as f64 / scaling_factor);
	}
	Ok(())
}

pub async fn publish_tokio_metrics(
	mut metrics_iterators: HashMap<String, impl Iterator<Item = TaskMetrics>>,
) -> Result<(), ServiceError<Error>> {
	let frequency = Duration::from_millis(TOKIO_POLLING_INTERVAL_MS);
	loop {
		for (key, val) in metrics_iterators.iter_mut() {
			if let Some(task_metrics) = val.next() {
				let label = HashMap::<&str, &str>::from([(TASK_NAME, &key[..])]);
				MEAN_IDLE_DURATION
					.with(&label)
					.set(task_metrics.mean_idle_duration().as_millis() as i64);
				MEAN_POLL_DURATION
					.with(&label)
					.set(task_metrics.mean_poll_duration().as_millis() as i64);
				MEAN_SCHEDULED_DURATION
					.with(&label)
					.set(task_metrics.mean_scheduled_duration().as_millis() as i64);
			}
		}
		tokio::time::sleep(frequency).await;
	}
}
