use crate::stellar_oracle::{
    EnvelopesMap, Error, FilterTx, FilterWith, ScpMessageCollector, TxHashMap, TxSetCheckerMap, TxSetMap,
};
use std::collections::HashMap;
use stellar_relay::{sdk::types::StellarMessage, StellarNodeMessage, UserControls};
use tokio::sync::{mpsc, oneshot};

pub struct ScpMessageMaps {
    envelopes_map: EnvelopesMap,
    txset_map: TxSetMap,
    tx_hash_map: TxHashMap,
}

impl ScpMessageMaps {
    fn new(collector: &ScpMessageCollector) -> Self {
        ScpMessageMaps {
            envelopes_map: collector.envelopes_map().clone(),
            txset_map: collector.txset_map().clone(),
            tx_hash_map: collector.tx_hash_map().clone(),
        }
    }

    pub fn envelopes_map(&self) -> &EnvelopesMap {
        &self.envelopes_map
    }

    pub fn txset_map(&self) -> &TxSetMap {
        &self.txset_map
    }

    pub fn tx_hash_map(&self) -> &TxHashMap {
        &self.tx_hash_map
    }
}

pub enum Message {
    GetCurrentMap { sender: oneshot::Sender<ScpMessageMaps> },
}
pub struct Actor {
    receiver: mpsc::Receiver<Message>,
}

impl Actor {
    pub fn new(receiver: mpsc::Receiver<Message>) -> Self {
        Actor { receiver }
    }

    async fn handle_message(&mut self, msg: Message, collector: &ScpMessageCollector) {
        match msg {
            Message::GetCurrentMap { sender } => {
                let _ = sender.send(ScpMessageMaps::new(collector));
            }
        };
    }

    async fn run(&mut self, mut user: UserControls, mut collector: ScpMessageCollector) -> Result<(), Error> {
        let mut tx_set_hash_map: TxSetCheckerMap = HashMap::new();

        loop {
            tokio::select! {
                Some(conn_state) = user.recv() => {
                    match conn_state {
                        StellarNodeMessage::Data {
                            p_id: _,
                            msg_type: _,
                            msg,
                        } => match msg {
                            StellarMessage::ScpMessage(env) => {
                                collector
                                    .handle_envelope(env, &mut tx_set_hash_map, &user)
                                    .await?;
                            }
                            StellarMessage::TxSet(set) => {
                                collector.handle_tx_set(&set, &mut tx_set_hash_map, &FilterTx).await?;
                            }
                            _ => {}
                        },

                        _ => {}
                    }
                }
                Some(msg) = self.receiver.recv() => {
                        self.handle_message(msg,&mut collector).await;
                    }
            }
        }
    }
}

pub struct ActorHandler {
    sender: mpsc::Sender<Message>,
}

impl ActorHandler {
    pub fn new(mut user: UserControls, mut collector: ScpMessageCollector) -> Self {
        let (sender, receiver) = mpsc::channel(1024);
        let mut actor = Actor::new(receiver);
        tokio::spawn(async move { actor.run(user, collector).await });

        Self { sender }
    }

    pub async fn get_map(&self, sender: oneshot::Sender<ScpMessageMaps>) -> Result<(), Error> {
        self.sender
            .send(Message::GetCurrentMap { sender })
            .await
            .map_err(Error::from)
    }
}
