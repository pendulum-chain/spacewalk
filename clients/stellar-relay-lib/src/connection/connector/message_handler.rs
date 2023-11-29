use substrate_stellar_sdk::types::{Hello, MessageType, StellarMessage};
use substrate_stellar_sdk::XdrCodec;

use crate::connection::{Connector, Xdr, Error, helper::{error_to_string, time_now}, xdr_converter::parse_authenticated_message};
use crate::connection::authentication::verify_remote_auth_cert;
use crate::connection::hmac::HMacKeys;

use crate::node::RemoteInfo;

impl Connector {
    /// Processes the raw bytes from the stream
    pub(super) async fn process_raw_message(&mut self, data: Xdr) -> Result<Option<StellarMessage>, Error> {
        let (auth_msg, msg_type) = parse_authenticated_message(&data)?;

        match msg_type {
            MessageType::Transaction | MessageType::FloodAdvert if !self.receive_tx_messages() => {
                self.increment_remote_sequence()?;
                self.check_to_send_more(MessageType::Transaction).await?;
            },

            MessageType::ScpMessage if !self.receive_scp_messages() => {
                self.increment_remote_sequence()?;
            },

            MessageType::ErrorMsg => match auth_msg.message {
                StellarMessage::ErrorMsg(e) => {
                    log::error!(
						"process_raw_message(): Received ErrorMsg during authentication: {}",
						error_to_string(e.clone())
					);
                    return Err(Error::OverlayError(e.code))
                },
                other => log::error!("process_raw_message(): Received ErroMsg during authentication: {:?}", other),
            },

            _ => {
                // we only verify the authenticated message when a handshake has been done.
                if self.is_handshake_created() {
                    self.verify_auth(&auth_msg, &data[4..(data.len() - 32)])?;
                    self.increment_remote_sequence()?;
                    log::trace!(
						"process_raw_message(): Processing {msg_type:?} message: auth verified"
					);
                }

                return self.process_stellar_message( auth_msg.message, msg_type).await;
            },
        }
        Ok(None)
    }

    /// Returns a StellarMessage for the user/outsider. Else none if user/outsider do not need it.
    /// This handles what to do with the Stellar message.
    async fn process_stellar_message(
        &mut self,
        msg: StellarMessage,
        msg_type: MessageType,
    ) -> Result<Option<StellarMessage>, Error> {
        match msg {
            StellarMessage::Hello(hello) => {
                // update the node info based on the hello message
                self.process_hello_message(hello)?;

                self.got_hello();

                if self.remote_called_us() {
                    self.send_hello_message().await?;
                } else {
                    self.send_auth_message().await?;
                }
                log::info!("process_stellar_message(): Hello message processed successfully");
            },

            StellarMessage::Auth(_) => {
                self.process_auth_message().await?;
            },

            StellarMessage::ErrorMsg(e) => {
                log::error!("process_stellar_message(): received from overlay: {e:?}");
                return Ok(Some(StellarMessage::ErrorMsg(e)));
                // self.send_to_user(StellarMessage::ErrorMsg(e)).await?;
            },

            other => {
                log::trace!(
					"process_stellar_message():  Processing {other:?} message: received from overlay"
				);
                self.check_to_send_more(msg_type).await?;
                return Ok(Some(other));
            },
        }

        Ok(None)
    }

    async fn process_auth_message(&mut self) -> Result<(), Error> {
        if self.remote_called_us() {
            self.send_auth_message().await?;
        }

        self.handshake_completed();

        if let Some(remote) = self.remote() {
            log::debug!("process_auth_message(): sending connect message: {remote:?}");
            self.enable_flow_controller(
                self.local().node().overlay_version,
                remote.node().overlay_version,
            );
        } else {
            log::warn!("process_auth_message(): No remote overlay version after handshake.");
        }

        self.check_to_send_more(MessageType::Auth).await
    }

    /// Updates the config based on the hello message that was received from the Stellar Node
    fn process_hello_message(&mut self, hello: Hello) -> Result<(), Error> {
        let mut network_id = self.connection_auth.network_id().to_xdr();

        if !verify_remote_auth_cert(time_now(), &hello.peer_id, &hello.cert, &mut network_id) {
            return Err(Error::AuthCertInvalid)
        }

        let remote_info = RemoteInfo::new(&hello);
        let shared_key = self.get_shared_key(remote_info.pub_key_ecdh());

        self.set_hmac_keys(HMacKeys::new(
            &shared_key,
            self.local().nonce(),
            remote_info.nonce(),
            self.remote_called_us(),
        ));

        self.set_remote(remote_info);

        Ok(())
    }
}
