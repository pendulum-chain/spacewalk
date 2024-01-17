use crate::connection::{xdr_converter::get_xdr_message_length, Connector, Error, Xdr};
use std::{
	io::Read,
	net::{Shutdown, TcpStream},
};
use substrate_stellar_sdk::types::StellarMessage;

use tokio::sync::{mpsc, mpsc::error::TryRecvError};

/// Polls for messages coming from the Stellar Node and communicates it back to the user
///
/// # Arguments
/// * `connector` - contains the config and necessary info for connecting to Stellar Node
/// * `read_stream_overlay` - the read half of the stream that is connected to Stellar Node
/// * `send_to_user_sender` - sends message from Stellar to the user
/// * `send_to_node_receiver` - receives message from user and writes it to the write half of the
///   stream.
pub(crate) async fn poll_messages_from_stellar(
	mut connector: Connector,
	send_to_user_sender: mpsc::Sender<StellarMessage>,
	mut send_to_node_receiver: mpsc::Receiver<StellarMessage>,
) {
	log::info!("poll_messages_from_stellar(): started.");

	loop {
		log::trace!("poll_messages_from_stellar(): start loop");
		if send_to_user_sender.is_closed() {
			log::info!("poll_messages_from_stellar(): closing receiver during disconnection");
			// close this channel as communication to user was closed.
			break
		}

		log::trace!("poll_messages_from_stellar(): checking for messages from the user...");
		// check for messages from user.
		match send_to_node_receiver.try_recv() {
			Ok(msg) =>
				if let Err(e) = connector.send_to_node(msg) {
					log::error!("poll_messages_from_stellar(): Error occurred during sending message to node: {e:?}");
				},
			Err(TryRecvError::Disconnected) => {
				log::trace!("poll_messages_from_stellar(): Recv channel (for sending message to node) got disconnected.");
				break
			},
			Err(TryRecvError::Empty) => {
				log::trace!("poll_messages_from_stellar(): Recv channel (for sending message to node) received empty messages.");
			},
		}

		let stream_clone =
			match connector.tcp_stream.try_clone() {
				Err(e) => {
					log::error!("poll_messages_from_stellar(): Error occurred during cloning tcp stream: {e:?}");
					break
				},
				Ok(stream_clone) => stream_clone,
			};

		log::trace!("poll_messages_from_stellar(): checking for messages from Stellar Node...");

		// check for messages from Stellar Node.
		let xdr = match read_message_from_stellar(stream_clone) {
			Err(e) => {
				log::error!("poll_messages_from_stellar(): {e:?}");
				break
			},
			Ok(xdr) => xdr,
		};

		match connector.process_raw_message(xdr).await {
			Ok(Some(stellar_msg)) =>
			// push message to user
				if let Err(e) = send_to_user_sender.send(stellar_msg).await {
					log::warn!("poll_messages_from_stellar(): Error occurred during sending message to user: {e:?}");
				},
			Ok(None) => log::trace!("poll_messages_from_stellar(): No message to send to user."),
			Err(e) => {
				log::error!("poll_messages_from_stellar(): Error occurred during processing xdr message: {e:?}");
				break
			},
		}
	}

	log::trace!("poll_messages_from_stellar(): stop polling for messages...");
	// make sure to drop/shutdown the stream
	if let Err(e) = connector.tcp_stream.shutdown(Shutdown::Both) {
		log::error!("poll_messages_from_stellar(): Failed to shutdown the tcp stream: {e:?}");
	};

	send_to_node_receiver.close();
	drop(send_to_user_sender);

	log::debug!("poll_messages_from_stellar(): stopped.");
}

/// Returns Xdr format of the `StellarMessage` sent from the Stellar Node
fn read_message_from_stellar(mut stream: TcpStream) -> Result<Xdr, Error> {
	log::trace!("read_message_from_stellar(): start");

	// holds the number of bytes that were missing from the previous stellar message.
	let mut lack_bytes_from_prev = 0;
	let mut readbuf: Vec<u8> = vec![];

	let mut buff_for_peeking = vec![0; 4];
	loop {
		let stream_clone = stream.try_clone()?;
		// check whether or not we should read the bytes as:
		// 1. the length of the next stellar message
		// 2. the remaining bytes of the previous stellar message
		match stream.read(&mut buff_for_peeking) {
			Ok(size) if size == 0 => continue,
			Ok(_) if lack_bytes_from_prev == 0 => {
				// if there are no more bytes lacking from the previous message,
				// then check the size of next stellar message.
				// If it's not enough, skip it.
				let expect_msg_len = get_xdr_message_length(&buff_for_peeking);

				log::trace!(
					"read_message_from_stellar(): The next message length is {expect_msg_len}"
				);

				if expect_msg_len == 0 {
					// there's nothing to read; wait for the next iteration
					log::trace!(
						"read_message_from_stellar(): Nothing left to read; waiting for next loop"
					);
					continue
				}

				// let's start reading the actual stellar message.
				readbuf = vec![0; expect_msg_len];

				match read_message(
					stream_clone,
					&mut lack_bytes_from_prev,
					&mut readbuf,
					expect_msg_len,
				) {
					Ok(None) => continue,
					Ok(Some(xdr)) => return Ok(xdr),
					Err(e) => {
						log::trace!("read_message_from_stellar(): ERROR: {e:?}");
						return Err(e)
					},
				}
			},
			Ok(size) => {
				// The next few bytes was read. Add it to the readbuf.
				lack_bytes_from_prev -= size;
				readbuf.append(&mut buff_for_peeking);

				// let's read the continuation number of bytes from the previous message.
				match read_unfinished_message(stream_clone, &mut lack_bytes_from_prev, &mut readbuf)
				{
					Ok(None) => continue,
					Ok(Some(xdr)) => return Ok(xdr),
					Err(e) => {
						log::trace!("read_message_from_stellar(): ERROR: {e:?}");
						return Err(e)
					},
				}
			},

			Err(e) => {
				log::trace!("read_message_from_stellar(): ERROR peeking for messages: {e:?}");
				return Err(Error::ReadFailed(e.to_string()))
			},
		}
	}
}

/// Returns Xdr when all bytes from the stream have successfully been converted; else None.
/// This reads a number of bytes based on the expected message length.
///
/// # Arguments
/// * `r_stream` - the read stream for reading the xdr stellar message
/// * `lack_bytes_from_prev` - the number of bytes remaining, to complete the previous message
/// * `readbuf` - the buffer that holds the bytes of the previous and incomplete message
/// * `xpect_msg_len` - the expected # of bytes of the Stellar message
fn read_message(
	mut stream: TcpStream,
	lack_bytes_from_prev: &mut usize,
	readbuf: &mut Vec<u8>,
	xpect_msg_len: usize,
) -> Result<Option<Xdr>, Error> {
	let actual_msg_len = stream.read(readbuf).map_err(|e| Error::ReadFailed(e.to_string()))?;

	// only when the message has the exact expected size bytes, should we send to user.
	if actual_msg_len == xpect_msg_len {
		return Ok(Some(readbuf.clone()))
	}

	// The next bytes are remnants from the previous stellar message.
	// save it and read it on the next loop.
	*lack_bytes_from_prev = xpect_msg_len - actual_msg_len;
	*readbuf = readbuf[0..actual_msg_len].to_owned();
	log::trace!(
		"read_message(): received only partial message. Need {lack_bytes_from_prev} bytes to complete."
	);

	Ok(None)
}

/// Returns Xdr when all bytes from the stream have successfully been converted; else None.
/// Reads a continuation of bytes that belong to the previous message
///
/// # Arguments
/// * `r_stream` - the read stream for reading the xdr stellar message
/// * `lack_bytes_from_prev` - the number of bytes remaining, to complete the previous message
/// * `readbuf` - the buffer that holds the bytes of the previous and incomplete message
fn read_unfinished_message(
	mut stream: TcpStream,
	lack_bytes_from_prev: &mut usize,
	readbuf: &mut Vec<u8>,
) -> Result<Option<Xdr>, Error> {
	// let's read the continuation number of bytes from the previous message.
	let mut cont_buf = vec![0; *lack_bytes_from_prev];

	let actual_msg_len =
		stream.read(&mut cont_buf).map_err(|e| Error::ReadFailed(e.to_string()))?;

	// this partial message completes the previous message.
	if actual_msg_len == *lack_bytes_from_prev {
		log::trace!("read_unfinished_message(): received continuation from the previous message.");
		readbuf.append(&mut cont_buf);

		return Ok(Some(readbuf.clone()))
	}

	// this partial message is not enough to complete the previous message.
	if actual_msg_len > 0 {
		*lack_bytes_from_prev -= actual_msg_len;
		cont_buf = cont_buf[0..actual_msg_len].to_owned();
		readbuf.append(&mut cont_buf);
		log::trace!(
            "read_unfinished_message(): not enough bytes to complete the previous message. Need {lack_bytes_from_prev} bytes to complete."
        );
	}

	Ok(None)
}
