use std::str::FromStr;

use clap::Parser;

#[derive(Clone, Debug)]
pub enum RestartPolicy {
	Never,
	Always,
}

impl FromStr for RestartPolicy {
	type Err = String;
	fn from_str(code: &str) -> Result<Self, Self::Err> {
		match code {
			"never" => Ok(RestartPolicy::Never),
			"always" => Ok(RestartPolicy::Always),
			_ => Err("Could not parse input as RestartPolicy".to_string()),
		}
	}
}

#[derive(Clone, Debug)]
pub enum LoggingFormat {
	Full,
	Json,
}

impl Default for LoggingFormat {
	fn default() -> Self {
		LoggingFormat::Full
	}
}

impl FromStr for LoggingFormat {
	type Err = String;
	fn from_str(code: &str) -> Result<Self, Self::Err> {
		match code {
			"full" => Ok(LoggingFormat::Full),
			"json" => Ok(LoggingFormat::Json),
			_ => Err("Could not parse input as LoggingFormat".to_string()),
		}
	}
}

impl LoggingFormat {
	pub fn init_subscriber(&self) {
		match *self {
			Self::Full => crate::trace::init_subscriber(),
			Self::Json => crate::trace::init_json_subscriber(),
		}
	}
}

#[derive(Parser, Debug, Clone)]
pub struct ServiceConfig {
	/// Restart or stop on error.
	#[clap(long, env = "RESTART_POLICY", default_value = "always")]
	pub restart_policy: RestartPolicy,

	/// Logging output format.
	#[clap(long, env = "LOGGING_FORMAT", default_value = "full")]
	pub logging_format: LoggingFormat,
}

#[derive(Parser, Debug, Clone)]
pub struct MonitoringConfig {
	/// Do not expose a Prometheus metric endpoint.
	#[clap(long, env = "NO_PROMETHEUS")]
	pub no_prometheus: bool,

	/// Expose Prometheus exporter on all interfaces.
	///
	/// Default is local.
	#[clap(long, env = "PROMETHEUS_EXTERNAL")]
	pub prometheus_external: bool,

	/// Specify Prometheus exporter TCP Port.
	#[clap(long, env = "PROMETHEUS_PORT", default_value = "9616")]
	pub prometheus_port: u16,
}
