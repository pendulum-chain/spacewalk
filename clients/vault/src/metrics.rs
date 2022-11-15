use std::{collections::HashMap, time::Duration};

use tokio_metrics::TaskMetrics;

use service::Error as ServiceError;

use crate::Error;

pub async fn publish_tokio_metrics(
	mut metrics_iterators: HashMap<String, impl Iterator<Item = TaskMetrics>>,
) -> Result<(), ServiceError<Error>> {
	// TODO
	Ok(())
}
