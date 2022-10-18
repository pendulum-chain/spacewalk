use crate::connection::connector::{Connector, ConnectorActions};
use substrate_stellar_sdk::types::{MessageType, SendMore, StellarMessage};

use crate::connection::flow_controller::MAX_FLOOD_MSG_CAP;
use crate::handshake::create_auth_message;
use crate::Error;

impl Connector {
    /// Sends an xdr version of a wrapped AuthenticatedMessage ( StellarMessage ).
    async fn send_stellar_message(&mut self, msg: StellarMessage) -> Result<(), Error> {
        self.send_to_node(ConnectorActions::SendMessage(msg)).await
    }

    pub(super) async fn check_to_send_more(
        &mut self,
        message_type: MessageType,
    ) -> Result<(), Error> {
        if !self.inner_check_to_send_more(message_type) {
            return Ok(());
        }

        log::debug!("Sending Send More message...");
        let msg = StellarMessage::SendMore(SendMore {
            num_messages: MAX_FLOOD_MSG_CAP,
        });
        self.send_stellar_message(msg).await
    }

    pub(super) async fn send_hello_message(&mut self) -> Result<(), Error> {
        self.send_to_node(ConnectorActions::SendHello)
            .await
            .map_err(Error::from)
    }

    pub(super) async fn send_auth_message(&mut self) -> Result<(), Error> {
        self.send_stellar_message(create_auth_message()).await
    }
}
