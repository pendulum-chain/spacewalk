use std::time::Duration;

use jsonrpsee::{
	client_transport::ws::{Receiver, Sender, Url, WsTransportClientBuilder},
	core::client::{Client, ClientBuilder},
};
use tokio::time::{sleep, timeout};

use crate::{error::JsonRpseeError, Error};

const RETRY_TIMEOUT: Duration = Duration::from_millis(1000);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

async fn ws_transport(url: &str) -> Result<(Sender, Receiver), Error> {
	let url: Url = url.parse::<Url>().map_err(Error::UrlParseError)?;

	WsTransportClientBuilder::default()
		.build(url)
		.await
		.map_err(|e| Error::JsonRpseeError(JsonRpseeError::Transport(e.into())))
}

pub(crate) async fn new_websocket_client(
	url: &str,
	max_concurrent_requests: Option<usize>,
) -> Result<Client, Error> {
	let (sender, receiver) = ws_transport(url).await?;
	let ws_client = ClientBuilder::default()
		.request_timeout(CONNECTION_TIMEOUT)
		.max_concurrent_requests(max_concurrent_requests.unwrap_or(1024))
		.build_with_tokio(sender, receiver);

	Ok(ws_client)
}

pub(crate) async fn new_websocket_client_with_retry(
	url: &str,
	max_concurrent_requests: Option<usize>,
	connection_timeout: Duration,
) -> Result<Client, Error> {
	log::info!("Connecting to the spacewalk-parachain...");
	timeout(connection_timeout, async move {
		loop {
			match new_websocket_client(url, max_concurrent_requests).await {
				Err(err) if err.is_ws_invalid_url_error() => return Err(err),
				Err(Error::JsonRpseeError(JsonRpseeError::Transport(err))) => {
					log::trace!("could not connect to parachain: {}", err);
					sleep(RETRY_TIMEOUT).await;
					continue;
				},
				Ok(rpc) => {
					log::info!("Connected!");
					return Ok(rpc);
				},
				Err(err) => return Err(err),
			}
		}
	})
	.await?
}
