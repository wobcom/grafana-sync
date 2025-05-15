use serde::{Deserialize, Serialize};

use crate::FederationPeer;

pub trait PingPong {
    fn ping() -> Self;
    fn pong() -> Self;
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum MessageMeta<M: PingPong> {
    Ack,
    Message(M),
}

#[derive(Serialize, Deserialize)]
pub struct PeerMessage<M: PingPong>
{
    id: u64,
    data: MessageMeta<M>,
}

impl<M: PingPong> PeerMessage<M> {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn msg(&self) -> Option<&M> {
        match &self.data {
            MessageMeta::Ack => None,
            MessageMeta::Message(msg) => Some(msg),
        }
    }

    pub fn into_msg(self) -> Option<M> {
        match self.data {
            MessageMeta::Ack => None,
            MessageMeta::Message(msg) => Some(msg),
        }
    }

    pub fn as_msg(&self) -> Option<&M> {
        match &self.data {
            MessageMeta::Ack => None,
            MessageMeta::Message(msg) => Some(msg),
        }
    }

    pub fn is_ack(&self) -> bool {
        matches!(self.data, MessageMeta::Ack)
    }
}

impl<M> From<M> for PeerMessage<M> 
where
    M: serde::de::DeserializeOwned + serde::Serialize + PingPong
{
    fn from(data: M) -> Self {
        PeerMessage {
            id: rand::random(),
            data: MessageMeta::Message(data)
        }
    }
}

impl<M> From<&M> for PeerMessage<M>
where
    M: serde::de::DeserializeOwned + serde::Serialize + Clone + PingPong
{
    fn from(data: &M) -> Self {
        PeerMessage {
            id: rand::random(),
            data: MessageMeta::Message(data.clone()),
        }
    }
}

impl std::fmt::Debug for FederationPeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FederationPeer")
            .field("uid", &self.uid)
            .field("address", &self.address)
            .finish()
    }
}
