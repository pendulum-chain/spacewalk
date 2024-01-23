use std::net::Shutdown;

use crate::connection::{xdr_converter::get_xdr_message_length, Connector, Error, Xdr};
use async_std::io::ReadExt;
use substrate_stellar_sdk::{types::StellarMessage, XdrCodec};
use tokio::{
	io::AsyncReadExt,
	sync::{mpsc, mpsc::error::TryRecvError},
	time::{sleep, Duration},
};

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
	log::info!("poll_messages_from_stellar(): started.");
	let mut counter = 0;

	loop {
		
		if send_to_user_sender.is_closed() {
			log::info!("poll_messages_from_stellar(): closing receiver during disconnection");
			// close this channel as communication to user was closed.
			break
		}

		// just for testing, remove me
		// log::info!("poll loop");
		//log_counter(&mut counter).await;

		// check for messages from user.
		match send_to_node_receiver.try_recv() {
			Ok(msg) =>
				if let Err(e) = connector.send_to_node(msg).await {
					log::error!("poll_messages_from_stellar(): Error occurred during sending message to node: {e:?}");
				},
			Err(TryRecvError::Disconnected) => {
				log::info!("poll_messages_from_stellar(): Recv channel (for sending message to node) got disconnected.");
				break
			},
			Err(TryRecvError::Empty) => {},
		}

		// check for messages from Stellar Node.
		let xdr = match read_message_from_stellar(&mut connector).await {
			Err(e) => {
				log::error!("poll_messages_from_stellar(): {e:?}");
				break
			},
			Ok(xdr) => xdr,
		};

		match connector.process_raw_message(xdr).await {
			Ok(Some(stellar_msg)) => {
				// push message to user
				if let Err(e) = send_to_user_sender.send(stellar_msg.clone()).await {
					log::warn!("poll_messages_from_stellar(): Error occurred during sending message {} to user: {e:?}",
						String::from_utf8(stellar_msg.to_base64_xdr())
						.unwrap_or_else(|_| format!("{:?}", stellar_msg.to_base64_xdr()))
					);
				}
			},
			Ok(None) => {},
			Err(e) => {
				log::error!("poll_messages_from_stellar(): Error occurred during processing xdr message: {e:?}");
				break
			},
		}
	}

	// make sure to shutdown the stream
	if let Err(e) = connector.tcp_stream.shutdown(Shutdown::Both) {
		log::error!("poll_messages_from_stellar(): Failed to shutdown the tcp stream: {e:?}");
	};
	send_to_node_receiver.close();
	drop(send_to_user_sender);

	log::info!("poll_messages_from_stellar(): stopped.");
}

// just for testing the outer loop is getting polled.
// If the counter increases, means the yield from read_message_from_stellar
// is working.
async fn log_counter(counter: &mut u32) {
	*counter += 1;
	log::info!("Outer loop iteration: {}", *counter);
	tokio::time::sleep(Duration::from_millis(100)).await;
}

/// Returns Xdr format of the `StellarMessage` sent from the Stellar Node
async fn read_message_from_stellar(connector: &mut Connector) -> Result<Xdr, Error> {
	// holds the number of bytes that were missing from the previous stellar message.
	let mut lack_bytes_from_prev = 0;
	let mut readbuf: Vec<u8> = vec![];
	let mut buff_for_reading = vec![0; 4];
	loop {
		//log::info!("read loop");
		// check whether or not we should read the bytes as:
		// 1. the length of the next stellar message
		// 2. the remaining bytes of the previous stellar message
		match connector.tcp_stream.read(&mut buff_for_reading).await {
			Ok(size) if size == 0 => {
				//if we remove the yield here, we have the risk that the process will become a 
				// "busy one", never letting other processes handle the exit
				tokio::task::yield_now().await;
				sleep(Duration::from_millis(100)).await;
				continue
			},
			Ok(_) if lack_bytes_from_prev == 0 => {
				// if there are no more bytes lacking from the previous message,
				// then check the size of next stellar message.
				let expect_msg_len = get_xdr_message_length(&buff_for_reading);

				// If it's not enough, skip it.
				if expect_msg_len == 0 {
					// there's nothing to read; wait for the next iteration
					log::info!("read_message_from_stellar(): expect_msg_len == 0");
					tokio::task::yield_now().await;
					sleep(Duration::from_millis(100)).await;
					continue
				}

				// let's start reading the actual stellar message.
				readbuf = vec![0; expect_msg_len];

				match read_message(
					connector,
					&mut lack_bytes_from_prev,
					&mut readbuf,
					expect_msg_len,
				)
				.await
				{
					Ok(None) => {
						continue
					},
					Ok(Some(xdr)) => return Ok(xdr),
					Err(e) => {
						log::info!("read_message_from_stellar(): ERROR: {e:?}");
						return Err(e)
					},
				}
			},
			Ok(size) => {
				// The next few bytes was read. Add it to the readbuf.
				lack_bytes_from_prev -= size;
				readbuf.append(&mut buff_for_reading);

				// let's read the continuation number of bytes from the previous message.
				match read_unfinished_message(connector, &mut lack_bytes_from_prev, &mut readbuf)
					.await
				{
					Ok(None) => {
						continue
					},
					Ok(Some(xdr)) => return Ok(xdr),
					Err(e) => {
						log::info!("read_message_from_stellar(): ERROR: {e:?}");
						return Err(e)
					},
				}
			},

			Err(e) => {
				log::info!("read_message_from_stellar(): ERROR reading messages: {e:?}");
				return Err(Error::ReadFailed(e.to_string()))
			},
		}
	}
}

/// Returns Xdr when all bytes from the stream have successfully been converted; else None.
/// This reads a number of bytes based on the expected message length.
///
/// # Arguments
/// * `stream` - the TcpStream for reading the xdr stellar message
/// * `lack_bytes_from_prev` - the number of bytes remaining, to complete the previous message
/// * `readbuf` - the buffer that holds the bytes of the previous and incomplete message
/// * `xpect_msg_len` - the expected # of bytes of the Stellar message
async fn read_message(
	connector: &mut Connector,
	lack_bytes_from_prev: &mut usize,
	readbuf: &mut Vec<u8>,
	xpect_msg_len: usize,
) -> Result<Option<Xdr>, Error> {
	let actual_msg_len = connector
		.tcp_stream
		.read(readbuf)
		.await
		.map_err(|e| Error::ReadFailed(e.to_string()))?;

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
/// * `stream` - the TcpStream for reading the xdr stellar message
/// * `lack_bytes_from_prev` - the number of bytes remaining, to complete the previous message
/// * `readbuf` - the buffer that holds the bytes of the previous and incomplete message
async fn read_unfinished_message(
	connector: &mut Connector,
	lack_bytes_from_prev: &mut usize,
	readbuf: &mut Vec<u8>,
) -> Result<Option<Xdr>, Error> {
	// let's read the continuation number of bytes from the previous message.
	let mut cont_buf = vec![0; *lack_bytes_from_prev];

	let actual_msg_len = connector
		.tcp_stream
		.read(&mut cont_buf)
		.await
		.map_err(|e| Error::ReadFailed(e.to_string()))?;

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
