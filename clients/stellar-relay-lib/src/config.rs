use crate::{connection::Error, ConnectorActions, StellarOverlayConnection, StellarRelayMessage};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, BytesOrString};
use std::fmt::{Debug, Formatter};
use tokio::sync::mpsc;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StellarOverlayConfig {
	connection_info: ConnectionInfoCfg,
	node_info: NodeInfoCfg,
}

impl StellarOverlayConfig {
	pub fn try_from_path(path: &str) -> Result<Self, Error> {
		let read_file = std::fs::read_to_string(path)
			.map_err(|e| Error::ConfigError(format!("File: {:?}", e)))?;
		serde_json::from_str(&read_file).map_err(|e| Error::ConfigError(format!("File: {:?}", e)))
	}
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInfoCfg {
	pub ledger_version: u32,
	pub overlay_version: u32,
	pub overlay_min_version: u32,
	#[serde_as(as = "BytesOrString")]
	pub version_str: Vec<u8>,
	pub is_pub_net: bool,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionInfoCfg {
	#[serde_as(as = "BytesOrString")]
	pub address: Vec<u8>,
	pub port: u32,

	#[serde(skip_serializing)]
	#[serde_as(as = "BytesOrString")]
	secret_key: Vec<u8>,

	#[serde(default = "ConnectionInfoCfg::default_auth_cert_exp")]
	pub auth_cert_expiration: u64,

	#[serde(default = "ConnectionInfoCfg::default_recv_tx_msgs")]
	pub recv_tx_msgs: bool,

	#[serde(default = "ConnectionInfoCfg::default_recv_scp_msgs")]
	pub recv_scp_msgs: bool,

	#[serde(default = "ConnectionInfoCfg::default_remote_called_us")]
	pub remote_called_us: bool,

	/// how long to wait for the Stellar Node's messages.
	#[serde(default = "ConnectionInfoCfg::default_timeout")]
	pub timeout_in_secs: u64,

	/// number of retries to wait for the Stellar Node's messages and/or to connect back to it.
	#[serde(default = "ConnectionInfoCfg::default_retries")]
	pub retries: u8,
}

impl ConnectionInfoCfg {
	fn default_auth_cert_exp() -> u64 {
		0
	}

	fn default_recv_tx_msgs() -> bool {
		false
	}

	fn default_recv_scp_msgs() -> bool {
		true
	}

	fn default_remote_called_us() -> bool {
		false
	}

	fn default_timeout() -> u64 {
		10
	}

	fn default_retries() -> u8 {
		3
	}

	pub fn secret_key(&self) -> &[u8] {
		self.secret_key.as_slice()
	}
}

/// Triggers connection to the Stellar Node.
/// Returns the UserControls for the user to send and receive Stellar messages.
pub async fn connect(cfg: StellarOverlayConfig) -> Result<StellarOverlayConnection, Error> {
	let local_node = cfg.node_info;
	let conn_info = cfg.connection_info;

	StellarOverlayConnection::connect(local_node.into(), conn_info.try_into()?).await
}

#[cfg(test)]
mod test {
	use super::*;
	use serde::de::Error;

	#[test]
	fn connection_info_conversion_successful() {
		let json = r#"
		 {
             "address":"1.2.3.4",
             "port":11625,
             "secret_key": "SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73",
		 }
		 "#;

		let _: ConnectionInfoCfg =
			serde_json::from_str(json).expect("should return a ConnectionInfoCfg");
	}

	#[test]
	fn missing_fields_in_connection_info_config() {
		// missing port
		let json = r#"
		 {
             "address":"1.2.3.4",
             "secret_key": "SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73",
             "recv_scp_msgs":true,
             "remote_called_us":false
		 }
		 "#;

		assert!(serde_json::from_str::<ConnectionInfoCfg>(json).is_err());

		// missing secret key
		let json = r#"
		 {
             "address":"1.2.3.4",
             "port":11625,
             "auth_cert_expiration":0,
             "recv_tx_msgs":false,
             "recv_scp_msgs":true
		 }
		 "#;
		assert!(serde_json::from_str::<ConnectionInfoCfg>(json).is_err());
	}

	#[test]
	fn node_info_conversion_successful() {
		let json = r#"
        {
			"ledger_version":19,
			"overlay_version":25,
			"overlay_min_version":23,
			"version_str":"v19.5.0",
			"is_pub_net": false
		}
        "#;

		let _: NodeInfoCfg = serde_json::from_str(json).expect("should return a NodeInfoCfg");
	}

	#[test]
	fn missing_fields_in_node_info_config() {
		// missing version_str
		let json = r#"
        {
			"ledger_version":19,
			"overlay_version":25,
			"overlay_min_version":23,
			"is_pub_net": false
		}
        "#;

		assert!(serde_json::from_str::<NodeInfoCfg>(json).is_err());

		// missing is_pub_net
		let json = r#"
        {
			"ledger_version":19,
			"overlay_version":25,
			"overlay_min_version":23,
			"version_str":"v19.5.0",
		}
        "#;

		assert!(serde_json::from_str::<NodeInfoCfg>(json).is_err());
	}

	#[test]
	fn stellar_relay_config_conversion_successful() {
		let json = r#"
        {
            "connection_info":{
                "address":"1.2.3.4",
                "port":11625,
                "secret_key": "SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73",
                "auth_cert_expiration":0,
                "recv_scp_msgs":true,
            },
            "node_info":{
                "ledger_version":19,
                "overlay_version":25,
                "overlay_min_version":23,
                "version_str":"v19.5.0",
                "is_pub_net":false
            }
        }
		"#;

		let _: StellarOverlayConfig =
			serde_json::from_str(json).expect("should return a NodeInfoCfg");
	}

	#[test]
	fn missing_fields_in_stellar_relay_config() {
		// missing address in Connection_Info, and overlay_min_version in Node_Info
		let json = r#"
        {
            "connection_info":{
                "port":11625,
                "secret_key": "SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73",
            },
            "node_info":{
                "ledger_version":19,
                "overlay_version":25,
                "version_str":"v19.5.0",
                "is_pub_net":false
            }
        }
		"#;

		assert!(serde_json::from_str::<StellarOverlayConfig>(json).is_err());

		// missing overlay_version in Node Info
		let json = r#"
        {
            "connection_info":{
                "address":"1.2.3.4",
                "port":11625,
                "secret_key": "SBLI7RKEJAEFGLZUBSCOFJHQBPFYIIPLBCKN7WVCWT4NEG2UJEW33N73",
                "auth_cert_expiration":0,
                "recv_scp_msgs":true,
            },
            "node_info":{
                "ledger_version":19,
                "overlay_min_version":23,
                "version_str":"v19.5.0",
                "is_pub_net":false
            }
        }
		"#;

		assert!(serde_json::from_str::<StellarOverlayConfig>(json).is_err());
	}
}
