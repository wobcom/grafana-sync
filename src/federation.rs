use grafana_sync_federation::{PeerMessage, PingPong};

use crate::instance::GrafanaInfo;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum SyncMessage {
    Ping,
    Pong,
    ConnectedGrafanas {
        instances: Vec<GrafanaInfo>,
    },
}

#[allow(dead_code)]
impl SyncMessage {
    pub fn into_message(self) -> PeerMessage<Self> {
        self.into()
    }

    pub fn as_message(&self) -> PeerMessage<Self> {
        self.into()
    }
}

impl PingPong for SyncMessage {
    fn ping() -> Self {
        Self::Ping
    }

    fn pong() -> Self {
        Self::Pong
    }
}

