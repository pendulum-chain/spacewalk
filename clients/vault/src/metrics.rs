use std::collections::HashMap;

use lazy_static::lazy_static;
use tokio_metrics::TaskMetrics;

use runtime::prometheus::IntCounter;
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
	// TODO: publish metrics to prometheus
	// We don't need this for now
	Ok(())
}
