use crate::connection::connector::ConnectorActions;
use crate::Error;
use crate::StellarNodeMessage;
use substrate_stellar_sdk::types::StellarMessage;
use tokio::sync::mpsc;

pub struct UserControls {
    /// This is when we want to send stellar messages
    tx: mpsc::Sender<ConnectorActions>,
    /// For receiving stellar messages
    rx: mpsc::Receiver<StellarNodeMessage>,
}

impl UserControls {
    pub fn new(tx: mpsc::Sender<ConnectorActions>, rx: mpsc::Receiver<StellarNodeMessage>) -> Self {
        UserControls { tx, rx }
    }
    pub async fn send(&self, message: StellarMessage) -> Result<(), Error> {
        self.tx
            .send(ConnectorActions::SendMessage(message))
            .await
            .map_err(Error::from)
    }

    /// Receives Stellar messages from the connection.
    pub async fn recv(&mut self) -> Option<StellarNodeMessage> {
        self.rx.recv().await
    }
}
