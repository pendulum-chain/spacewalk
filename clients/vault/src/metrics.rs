use std::{collections::HashMap, time::Duration};

use lazy_static::lazy_static;
use tokio_metrics::TaskMetrics;

use runtime::prometheus::{
	gather, proto::MetricFamily, Encoder, Gauge, GaugeVec, IntCounter, IntGauge, IntGaugeVec, Opts,
	Registry, TextEncoder,
};
use service::Error as ServiceError;

use crate::Error;

lazy_static! {
	pub static ref RESTART_COUNT: IntCounter =
		IntCounter::new("restart_count", "Number of service restarts")
			.expect("Failed to create prometheus metric");
}

pub fn increment_restart_counter() {
	RESTART_COUNT.inc();
}

pub async fn publish_tokio_metrics(
	mut metrics_iterators: HashMap<String, impl Iterator<Item = TaskMetrics>>,
) -> Result<(), ServiceError<Error>> {
	// TODO
	Ok(())
}
