use crate::connection::{xdr_converter::get_xdr_message_length, Connector, Error, Xdr};
use async_std::io::ReadExt;
use std::time::Duration;
use substrate_stellar_sdk::{types::StellarMessage, XdrCodec};
use tokio::{
	sync::{mpsc, mpsc::error::TryRecvError},
	time::timeout,
};
use tracing::{debug, error, info, trace, warn};

/// The waiting time for reading messages from stream.
static READ_TIMEOUT_IN_SECS: u64 = 60;

/// Polls for messages coming from the Stellar Node and communicates it back to the user
///
/// # Arguments
/// * `connector` - contains the config and necessary info for connecting to Stellar Node
/// * `send_to_user_sender` - sends message from Stellar to the user
/// * `send_to_node_receiver` - receives message from user and writes it to the write half of the
///   stream.
pub(crate) async fn poll_messages_from_stellar(
	mut connector: Connector,
	send_to_user_sender: mpsc::Sender<StellarMessage>,
	mut send_to_node_receiver: mpsc::Receiver<StellarMessage>,
) {
	info!("poll_messages_from_stellar(): started.");

	loop {
		if send_to_user_sender.is_closed() {
			info!("poll_messages_from_stellar(): closing receiver during disconnection");
			// close this channel as communication to user was closed.
			break;
		}

		// check for messages from user.
		match send_to_node_receiver.try_recv() {
			Ok(msg) =>
				if let Err(e) = connector.send_to_node(msg).await {
					error!("poll_messages_from_stellar(): Error occurred during sending message to node: {e:?}");
				},
			Err(TryRecvError::Disconnected) => break,
			Err(TryRecvError::Empty) => {},
		}

		// check for messages from Stellar Node.
		// if reading took too much time, flag it as "disconnected"
		let xdr = match timeout(
			Duration::from_secs(READ_TIMEOUT_IN_SECS),
			read_message_from_stellar(&mut connector)
		).await {
			Ok(Ok(xdr)) => xdr,
			Ok(Err(e)) => {
				error!("poll_messages_from_stellar(): {e:?}");
				break
			},
			Err(_) => {
				error!("poll_messages_from_stellar(): timed out");
				break
			}
		};

		match connector.process_raw_message(xdr).await {
			Ok(Some(stellar_msg)) => {
				// push message to user
				let stellar_msg_as_base64_xdr = stellar_msg.to_base64_xdr();
				if let Err(e) = send_to_user_sender.send(stellar_msg).await {
					warn!("poll_messages_from_stellar(): Error occurred during sending message {} to user: {e:?}",
					String::from_utf8(stellar_msg_as_base64_xdr.clone())
					.unwrap_or_else(|_| format!("{stellar_msg_as_base64_xdr:?}"))
				);
				}
				tokio::task::yield_now().await;
			},
			Ok(None) => {},
			Err(e) => {
				error!("poll_messages_from_stellar(): Error occurred during processing xdr message: {e:?}");
				break;
			},
		}
	}

	// make sure to shutdown the connector
	connector.stop();
	send_to_node_receiver.close();
	drop(send_to_user_sender);

	debug!("poll_messages_from_stellar(): stopped.");
}

/// Returns Xdr format of the `StellarMessage` sent from the Stellar Node
async fn read_message_from_stellar(connector: &mut Connector) -> Result<Xdr, Error> {
	// holds the number of bytes that were missing from the previous stellar message.
	let mut lack_bytes_from_prev = 0;
	let mut readbuf: Vec<u8> = vec![];
	let mut buff_for_reading = vec![0; 4];

	loop {
		// identify bytes as:
		//  1. the length of the next stellar message
		//  2. the remaining bytes of the previous stellar message
		// return Timeout error if reading time has elapsed.
		match connector.tcp_stream.read(&mut buff_for_reading)
		.await {
			Ok(0) => continue,
			Ok(_) if lack_bytes_from_prev == 0 =>  {
				// if there are no more bytes lacking from the previous message,
				// then check the size of next stellar message.
				let expect_msg_len = get_xdr_message_length(&buff_for_reading);

				// If it's not enough, skip it.
				if expect_msg_len == 0 {
					// there's nothing to read; wait for the next iteration
					trace!("read_message_from_stellar(): expect_msg_len == 0");
					continue;
				}

				// let's start reading the actual stellar message.
				readbuf = vec![0; expect_msg_len];

				match is_reading_complete(
					connector,
					&mut lack_bytes_from_prev,
					&mut readbuf,
					expect_msg_len,
				)
				.await
				{
					Ok(false) => continue,
					Ok(true) => return Ok(readbuf),
					Err(e) => {
						trace!("read_message_from_stellar(): ERROR: {e:?}");
						return Err(e)
					},
				}
			}
			Ok(size) => {
				// The next few bytes was read. Add it to the readbuf.
				lack_bytes_from_prev = lack_bytes_from_prev.saturating_sub(size);
				readbuf.append(&mut buff_for_reading);
				// make sure to cleanup the buffer
				buff_for_reading = vec![0; 4];

				// let's read the continuation number of bytes from the previous message.
				match is_reading_unfinished_message_complete(connector, &mut lack_bytes_from_prev, &mut readbuf)
					.await
				{
					Ok(false) => continue,
					Ok(true) => return Ok(readbuf),
					Err(e) => {
						trace!("read_message_from_stellar(): ERROR: {e:?}");
						return Err(e)
					},
				}
			}
			Err(e) => {
				trace!("read_message_from_stellar(): ERROR reading messages: {e:?}");
				return Err(Error::ReadFailed(e.to_string()))
			}
		}
	}
}

/// Returns true when all bytes from the stream have successfully been converted; else false.
/// This reads a number of bytes based on the expected message length.
///
/// # Arguments
/// * `connector` - a ref struct that contains the config and necessary info for connecting to
///   Stellar Node
/// * `lack_bytes_from_prev` - the number of bytes remaining, to complete the previous message
/// * `readbuf` - the buffer that holds the bytes of the previous and incomplete message
/// * `xpect_msg_len` - the expected # of bytes of the Stellar message
async fn is_reading_complete(
	connector: &mut Connector,
	lack_bytes_from_prev: &mut usize,
	readbuf: &mut Vec<u8>,
	xpect_msg_len: usize,
) -> Result<bool, Error> {
	let actual_msg_len = connector
		.tcp_stream
		.read(readbuf)
		.await
		.map_err(|e| Error::ReadFailed(e.to_string()))?;

	// only when the message has the exact expected size bytes, should we send to user.
	if actual_msg_len == xpect_msg_len {
		return Ok(true)
	}

	// The next bytes are remnants from the previous stellar message.
	// save it and read it on the next loop.
	*lack_bytes_from_prev = xpect_msg_len - actual_msg_len;
	*readbuf = readbuf[0..actual_msg_len].to_owned();
	trace!(
		"read_message(): received only partial message. Need {lack_bytes_from_prev} bytes to complete."
	);

	Ok(false)
}

/// Returns true when all bytes from the stream have successfully been converted; else false.
/// Reads a continuation of bytes that belong to the previous message
///
/// # Arguments
/// * `connector` - a ref struct that contains the config and necessary info for connecting to
///   Stellar Node
/// * `lack_bytes_from_prev` - the number of bytes remaining, to complete the previous message
/// * `readbuf` - the buffer that holds the bytes of the previous and incomplete message
async fn is_reading_unfinished_message_complete(
	connector: &mut Connector,
	lack_bytes_from_prev: &mut usize,
	readbuf: &mut Vec<u8>,
) -> Result<bool, Error> {
	// let's read the continuation number of bytes from the previous message.
	let mut cont_buf = vec![0; *lack_bytes_from_prev];

	let actual_msg_len = connector
		.tcp_stream
		.read(&mut cont_buf)
		.await
		.map_err(|e| Error::ReadFailed(e.to_string()))?;

	// this partial message completes the previous message.
	if actual_msg_len == *lack_bytes_from_prev {
		trace!("read_unfinished_message(): received continuation from the previous message.");
		readbuf.append(&mut cont_buf);

		return Ok(true)
	}

	// this partial message is not enough to complete the previous message.
	if actual_msg_len > 0 {
		*lack_bytes_from_prev -= actual_msg_len;
		cont_buf = cont_buf[0..actual_msg_len].to_owned();
		readbuf.append(&mut cont_buf);
		trace!(
            "read_unfinished_message(): not enough bytes to complete the previous message. Need {lack_bytes_from_prev} bytes to complete."
        );
	}

	Ok(false)
}
