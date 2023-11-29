use std::time::Duration;
use crate::{
	connection::{
		authentication::verify_remote_auth_cert, error_to_string, helper::time_now, hmac::HMacKeys,
		xdr_converter::{parse_authenticated_message, get_xdr_message_length}, Connector, Xdr,Error
	},
	node::RemoteInfo
};
use substrate_stellar_sdk::{
	types::{Hello, MessageType, StellarMessage},
	XdrCodec,
};
use substrate_stellar_sdk::types::ErrorCode;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp;
use tokio::sync::mpsc;
use tokio::time::timeout;
use crate::connection::to_base64_xdr_string;


/// Returns Xdr format of the `StellarMessage` sent from the Stellar Node
pub(super) async fn read_message_from_stellar(r_stream: &mut tcp::OwnedReadHalf, timeout_in_secs:u64) -> Option<Xdr> {
	let mut proc_id = 0;

	// holds the number of bytes that were missing from the previous stellar message.
	let mut lack_bytes_from_prev = 0;
	let mut readbuf: Vec<u8> = vec![];

	let mut buff_for_peeking = vec![0; 4];
	loop {
		// check whether or not we should read the bytes as:
		// 1. the length of the next stellar message
		// 2. the remaining bytes of the previous stellar message
		match timeout(Duration::from_secs(timeout_in_secs), r_stream.peek(&mut buff_for_peeking))
			.await
		{
			Ok(Ok(0)) => {
				log::error!("read_message_from_stellar(): proc_id: {proc_id}. Received 0 size");
				break
			}

			Ok(Ok(_)) if lack_bytes_from_prev == 0 => {
				// if there are no more bytes lacking from the previous message,
				// then check the size of next stellar message.
				// If it's not enough, skip it.
				let expect_msg_len = next_message_length(r_stream).await;
				log::trace!("read_message_from_stellar(): proc_id: {proc_id}. The next message length is {expect_msg_len}");

				if expect_msg_len == 0 {
					// there's nothing to read; wait for the next iteration
					log::trace!("read_message_from_stellar(): proc_id: {proc_id}. Nothing left to read; waiting for next loop");
					continue
				}

				// let's start reading the actual stellar message.
				readbuf = vec![0; expect_msg_len];

				match read_message(
					r_stream,
					&mut lack_bytes_from_prev,
					&mut proc_id,
					&mut readbuf,
					expect_msg_len,
				).await {
					Ok(None) => continue,
					Ok(Some(xdr)) => return Some(xdr),
					Err(e) => {
						log::error!("read_message_from_stellar(): proc_id: {proc_id}. encountered error: {e:?}");
						break
					}
				}
			}

			Ok(Ok(_)) => {
				// let's read the continuation number of bytes from the previous message.
				match read_unfinished_message(
					r_stream,
					&mut lack_bytes_from_prev,
					&mut proc_id,
					&mut readbuf,
				).await {
					Ok(None) => continue,
					Ok(Some(xdr)) => return Some(xdr),
					Err(e) => {
						log::error!("read_message_from_stellar(): proc_id: {proc_id}. encountered error: {e:?}");
						break
					}
				}
			}
			Ok(Err(e)) => {
				log::error!("read_message_from_stellar(): proc_id: {proc_id}. encountered error: {e:?}");
				break
			}

			Err(_) => {
				log::error!("read_message_from_stellar(): proc_id: {proc_id}. timeout elapsed.");
				break
			}
		}
	}

	None
}


/// Returns Xdr when all bytes from the stream have successfully been converted; else None.
/// This reads a number of bytes based on the expected message length.
///
/// # Arguments
/// * `r_stream` - the read stream for reading the xdr stellar message
/// * `lack_bytes_from_prev` - the number of bytes remaining, to complete the previous message
/// * `proc_id` - the process id, used for tracing.
/// * `readbuf` - the buffer that holds the bytes of the previous and incomplete message
/// * `xpect_msg_len` - the expected # of bytes of the Stellar message
async fn read_message(
	r_stream: &mut tcp::OwnedReadHalf,
	lack_bytes_from_prev: &mut usize,
	proc_id: &mut u32,
	readbuf: &mut Vec<u8>,
	xpect_msg_len: usize,
) -> Result<Option<Xdr>, Error> {
	let actual_msg_len = read_stream(r_stream, readbuf).await?;

	// only when the message has the exact expected size bytes, should we send to user.
	if actual_msg_len == xpect_msg_len {
		return Ok(Some(readbuf.clone()))
	}

	// The next bytes are remnants from the previous stellar message.
	// save it and read it on the next loop.
	*lack_bytes_from_prev = xpect_msg_len - actual_msg_len;
	*readbuf = readbuf[0..actual_msg_len].to_owned();
	log::trace!(
		"read_message(): proc_id: {} received only partial message. Need {} bytes to complete.",
		proc_id,
		lack_bytes_from_prev
	);

	Ok(None)
}

/// Returns Xdr when all bytes from the stream have successfully been converted; else None.
/// Reads a continuation of bytes that belong to the previous message
///
/// # Arguments
/// * `r_stream` - the read stream for reading the xdr stellar message
/// * `lack_bytes_from_prev` - the number of bytes remaining, to complete the previous message
/// * `proc_id` - the process id, used for tracing.
/// * `readbuf` - the buffer that holds the bytes of the previous and incomplete message
async fn read_unfinished_message(
	r_stream: &mut tcp::OwnedReadHalf,
	lack_bytes_from_prev: &mut usize,
	proc_id: &mut u32,
	readbuf: &mut Vec<u8>,
) -> Result<Option<Xdr>, Error> {
	// let's read the continuation number of bytes from the previous message.
	let mut cont_buf = vec![0; *lack_bytes_from_prev];

	let actual_msg_len = read_stream(r_stream, &mut cont_buf).await?;

	// this partial message completes the previous message.
	if actual_msg_len == *lack_bytes_from_prev {
		log::trace!("read_unfinished_message(): proc_id: {} received continuation from the previous message.", proc_id);
		readbuf.append(&mut cont_buf);

		return Ok(Some(readbuf.clone()));
	}

	// this partial message is not enough to complete the previous message.
	if actual_msg_len > 0 {
		*lack_bytes_from_prev -= actual_msg_len;
		cont_buf = cont_buf[0..actual_msg_len].to_owned();
		readbuf.append(&mut cont_buf);
		log::trace!(
            "read_unfinished_message(): proc_id: {} not enough bytes to complete the previous message. Need {} bytes to complete.",
            proc_id,
            lack_bytes_from_prev
        );
	}

	Ok(None)
}

/// checks the length of the next stellar message.
async fn next_message_length(r_stream: &mut tcp::OwnedReadHalf) -> usize {
	// let's check for messages.
	let mut sizebuf = [0; 4];

	if r_stream.read(&mut sizebuf).await.unwrap_or(0) == 0 {
		return 0
	}

	get_xdr_message_length(&sizebuf)
}

/// reads data from the stream and store to buffer
async fn read_stream(r_stream: &mut tcp::OwnedReadHalf, buffer: &mut [u8]) -> Result<usize, Error> {
	r_stream.read(buffer).await.map_err(|e| Error::ReadFailed(e.to_string()))
}
