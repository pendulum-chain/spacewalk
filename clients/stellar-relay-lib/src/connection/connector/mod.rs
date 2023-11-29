mod connector;
mod message_creation;
mod message_handler;
mod message_reader;
mod message_sender;

pub(crate) use connector::Connector;

use substrate_stellar_sdk::types::StellarMessage;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp;
use tokio::sync::mpsc;
use crate::connection::connector::message_reader::read_message_from_stellar;
use crate::connection::Error;

/// Polls for messages coming from the Stellar Node and communicates it back to the user
///
/// # Arguments
/// * `connector` - contains the config and necessary info for connecting to Stellar Node
/// * `r_stream` - the read half of the stream that is connected to Stellar Node
/// * `send_to_user_sender` - sends message from Stellar to the user
/// * `send_to_node_receiver` - receives message from user and writes it to the write half of the stream.
pub(crate) async fn poll_messages_from_stellar(
    mut connector: Connector,
    mut r_stream: tcp::OwnedReadHalf,
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
			opt_msg = read_message_from_stellar(&mut r_stream, connector.timeout_in_secs) => match opt_msg {
				None => break,
				Some(xdr) =>  match connector.process_raw_message(xdr).await {
                    Ok(Some(stellar_msg)) => {
                        // push message to user
                        if let Err(e) = send_to_user_sender.send(stellar_msg).await {
                            log::error!("poll_messages_from_stellar(): Error occurred during sending message to user: {e:?}");
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("poll_messages_from_stellar(): Error occurred during processing xdr message: {e:?}");
                        break;
                    }
                }
			},

			// push message to Stellar Node
			Some(msg) = send_to_node_receiver.recv() => if let Err(e) = connector.send_to_node(msg).await {
                log::error!("poll_messages_from_stellar(): Error occurred during sending message to node: {e:?}");
                break;
            }
		}
    }

    send_to_node_receiver.close();
    drop(send_to_user_sender);
    // make sure to drop/shutdown all streams and channels
    connector.wr.forget();
    drop(r_stream);

    log::info!("poll_messages_from_stellar(): stopped.");
}