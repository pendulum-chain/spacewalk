use crate::{
    connection::{
        authentication::verify_remote_auth_cert, helper::time_now, hmac::HMacKeys,
        xdr_converter::parse_authenticated_message, Connector, Xdr,
    },
    node::RemoteInfo,
    Error, StellarRelayMessage,
};
use substrate_stellar_sdk::{
    types::{Hello, MessageType, StellarMessage},
    XdrCodec,
};

impl Connector {
    /// Processes the raw bytes from the stream
    pub(crate) async fn process_raw_message(&mut self, xdr: Xdr) -> Result<(), Error> {
        let (proc_id, data) = xdr;
        let (auth_msg, msg_type) = parse_authenticated_message(&data)?;

        log::debug!("proc_id: {} processing {:?}", proc_id, msg_type);

        match msg_type {
            MessageType::Transaction if !self.receive_tx_messages() => {
                self.increment_remote_sequence()?;
                self.check_to_send_more(msg_type).await?;
            }

            MessageType::ScpMessage if !self.receive_scp_messages() => {
                self.increment_remote_sequence()?;
            }

            _ => {
                // we only verify the authenticated message when a handshake has been done.
                if self.is_handshake_created() {
                    self.verify_auth(&auth_msg, &data[4..(data.len() - 32)])?;
                    self.increment_remote_sequence()?;
                    log::trace!("proc_id: {}, auth message verified", proc_id);
                }

                self.process_stellar_message(proc_id, auth_msg.message, msg_type)
                    .await?;
            }
        }
        Ok(())
    }

    /// Handles what to do next with the Stellar message. Mostly it will be sent back to the user
    async fn process_stellar_message(
        &mut self,
        p_id: u32,
        msg: StellarMessage,
        msg_type: MessageType,
    ) -> Result<(), Error> {
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
                log::info!("Hello message processed successfully");
            }

            StellarMessage::Auth(_) => {
                self.process_auth_message().await?;
            }

            StellarMessage::SendMore(_) => {
                // todo: what to do with send more?
                log::trace!("what to do with send more");
            }
            other => {
                self.send_to_user(StellarRelayMessage::Data {
                    p_id,
                    msg_type,
                    msg: other,
                })
                .await?;
                self.check_to_send_more(msg_type).await?;
            }
        }
        Ok(())
    }

    async fn process_auth_message(&mut self) -> Result<(), Error> {
        if self.remote_called_us() {
            self.send_auth_message().await?;
        }

        self.handshake_completed();

        log::info!("Handshake completed");
        if let Some(remote) = self.remote() {
            self.send_to_user(StellarRelayMessage::Connect {
                pub_key: remote.pub_key().clone(),
                node_info: remote.node().clone(),
            })
            .await?;

            self.enable_flow_controller(self.local().node().overlay_version, remote.node().overlay_version);
        } else {
            log::warn!("No remote overlay version after handshake.");
        }

        self.check_to_send_more(MessageType::Auth).await
    }

    /// Updates the config based on the hello message that was received from the Stellar Node
    fn process_hello_message(&mut self, hello: Hello) -> Result<(), Error> {
        let mut network_id = self.connection_auth.network_id().to_xdr();

        if !verify_remote_auth_cert(time_now(), &hello.peer_id, &hello.cert, &mut network_id) {
            return Err(Error::AuthCertInvalid);
        }

        let remote_info = RemoteInfo::new(&hello);
        let shared_key = self.get_shared_key(&remote_info.pub_key_ecdh());

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
