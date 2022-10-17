use crate::connection::Xdr;
use substrate_stellar_sdk::types::StellarMessage;

mod connector;
mod message_creation;
mod message_handler;
mod message_sender;

pub(crate) use connector::Connector;

#[derive(Debug)]
pub enum ConnectorActions {
    SendHello,
    SendMessage(StellarMessage),
    HandleMessage(Xdr),
}
