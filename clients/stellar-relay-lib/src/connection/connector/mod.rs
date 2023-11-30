mod connector;
mod message_creation;
mod message_handler;
mod message_reader;
mod message_sender;


use std::time::Duration;
pub(crate) use connector::Connector;

use substrate_stellar_sdk::types::StellarMessage;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::net::{tcp, TcpStream};
use tokio::net::tcp::{OwnedReadHalf, ReuniteError};
use tokio::sync::mpsc;
use tokio::time::sleep;
use crate::connection::connector::message_reader::read_message_from_stellar;
use crate::connection::Error;
use crate::helper::{create_stream, to_base64_xdr_string};

/// Polls for messages coming from the Stellar Node and communicates it back to the user
///
/// # Arguments
/// * `connector` - contains the config and necessary info for connecting to Stellar Node
/// * `r_stream` - the read half of the stream that is connected to Stellar Node
/// * `send_to_user_sender` - sends message from Stellar to the user
/// * `send_to_node_receiver` - receives message from user and writes it to the write half of the stream.
pub(crate) async fn poll_messages_from_stellar(
    mut connector: Connector,
    mut r_stream: OwnedReadHalf,
    address: String,
    send_to_user_sender: mpsc::Sender<StellarMessage>,
    mut send_to_node_receiver: mpsc::Receiver<StellarMessage>
) {
    log::info!("poll_messages_from_stellar(): started.");

    loop {
        if send_to_user_sender.is_closed() {
            log::info!("poll_messages_from_stellar(): closing receiver during disconnection");
            // close this channel as communication to user was closed.
            break;
        }

        tokio::select! {
			result_msg = read_message_from_stellar(&mut r_stream, connector.timeout_in_secs) => match result_msg {
                Err(e) => {
                    log::error!("poll_messages_from_stellar(): {e:?}");
                    break;
                },
				Ok(xdr) =>  match connector.process_raw_message(xdr).await {
                    Ok(Some(stellar_msg)) =>
                    // push message to user
                    if let Err(e) = send_to_user_sender.send(stellar_msg).await {
                        log::warn!("poll_messages_from_stellar(): Error occurred during sending message to user: {e:?}");
                    },
                    Ok(_) => log::info!("poll_messages_from_stellar(): no message for user"),
                    Err(e) => {
                        log::error!("poll_messages_from_stellar(): Error occurred during processing xdr message: {e:?}");
                        break;
                    }
                }
			},

			// push message to Stellar Node
			Some(msg) = send_to_node_receiver.recv() => if let Err(e) = connector.send_to_node(msg).await {
                log::error!("poll_messages_from_stellar(): Error occurred during sending message to node: {e:?}");
            }
		}
    }
    // make sure to drop/shutdown the stream
    connector.wr.forget();
    drop(r_stream);

    send_to_node_receiver.close();
    drop(send_to_user_sender);


    log::info!("poll_messages_from_stellar(): stopped.");
}


async fn restart(connector: &mut Connector, address: &str) -> Result<OwnedReadHalf,Error> {
    // split the stream for easy handling of read and write
    let (rd, wr) = create_stream(address).await?;

    connector.wr = wr;
    Ok(rd)
}