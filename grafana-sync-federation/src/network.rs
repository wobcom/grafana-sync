use std::{collections::{HashMap, HashSet}, fmt::Debug, marker::PhantomData, net::SocketAddr, ops::{Deref, DerefMut}, path::PathBuf, sync::Arc, time::Duration};

use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use log::{debug, info, trace, warn};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{join, net::{TcpListener, TcpStream, ToSocketAddrs}};
use tokio_util::codec::Framed;

use crate::{authentication::AuthOrchestrator, encryption::EncryptionCodec, Error, FederationPeer, FrameError, PeerMessage, PeerUID, PingPong};

#[derive(Debug)]
pub struct FederatedNetwork<M> {
    local: Arc<FederationPeer>,
    peers: Arc<HashMap<String, FederationPeer>>,

    // uid, peer
    connected_peers: Arc<DashMap<String, PeerConnection<M>>>,
    listener: TcpListener,

    seen_message_ids: HashMap<PeerUID, HashSet<u64>>,

    auth_orchestrator: AuthOrchestrator<M>,
}

impl<M> FederatedNetwork<M> 
where
    M: PingPong 
        + serde::Serialize + serde::de::DeserializeOwned 
        + Sync + Send 
        + 'static 
{
    pub fn builder(local: FederationPeer, peers: Vec<FederationPeer>) -> FederatedNetworkBuilder {
        FederatedNetworkBuilder::new(local, peers)
    }

    pub async fn poll(&self) -> crate::Result<()> {
        let (stream, addr) = self.listener.accept().await?;

        debug!("{addr} connected");
        
        let mut conn: PeerConnection<M> = PeerConnection::new(&self.auth_orchestrator, stream, addr).await?;

        trace!("receiving: ping");
        tokio::select! {
            msg = conn.next_message() => msg,
            _ = tokio::time::sleep(Duration::from_secs(5)) => Err(Error::PeerTimeout),
        }?;


        conn.send(PeerMessage::from(M::pong())).await?;
        trace!("sent: pong");
    
        debug!("Peer {:?} successfully sent a message on an encrypted channel", conn.uid);

        info!("New peer connection with {:?} established and authenticated", conn.uid);

        self.connected_peers.insert(conn.uid.clone(), conn);

        if self.health() == NetworkHealth::Perfect {
            info!("Network is now in perfect health");
        }

        Ok(())
    }

    pub async fn run_tasks(self: Arc<FederatedNetwork<M>>) -> crate::Result<()> {
        let network = self.clone();
        let server = tokio::task::spawn(async move {
            loop {
                if let Err(e) = network.poll().await {
                    warn!("Error when accepting new client: {e}");
                }
            }
        });

        let network = self.clone();
        let client = tokio::task::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            loop {
                tick.tick().await;
                network.try_establish_perfect_network().await;
            }
        });

        let network = self.clone();
        let check_health = tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60 * 5)).await;
                if network.health() == NetworkHealth::Dead {
                    warn!("The network is dead.");
                }
            }
        });

        let (j1, j2, j3) = join!(server, client, check_health);
        j1?;
        j2?;
        j3?;

        Ok(())
    }

    // Returns true if a perfect network could be established.
    // Returns false if it's already in the best possible state
    pub async fn try_establish_perfect_network(&self) -> bool {
        if self.health() == NetworkHealth::Perfect {
            return false;
        }

        let disconnected_peers: Vec<_> = self.peers
            .iter()
            .filter(|(uid, _)| !self.connected_peers.contains_key(uid.as_str()))
            .map(|(_, p)| p)
            .collect();

        for peer in disconnected_peers {
            let result = self._connect_peer(peer).await;
            match result {
                Ok(_) => info!("New peer connection with {:?} established and authenticated", peer.uid),
                Err(Error::Io(_)) => (), // probably couldnt connect
                Err(e) => warn!("Error when establishing peer connection: {}", e),
            }
        }

        if self.health() == NetworkHealth::Perfect {
            info!("Network is now in perfect health");
        }

        self.health() == NetworkHealth::Perfect
    }

    pub async fn connect_peer(&self, peer: &FederationPeer) -> crate::Result<()> {
        if self.connected_peers.contains_key(&peer.uid) {
            return Ok(()); // already connected
        }

        self._connect_peer(peer).await
    }

    async fn _connect_peer(&self, peer: &FederationPeer) -> crate::Result<()> {
        debug!("Trying to connect to peer {:?}", peer.uid);

        let mut conn = PeerConnection::connect(&self.auth_orchestrator, peer.sock_addr()).await?;

        trace!("sent: ping");

        conn.send(PeerMessage::from(M::ping())).await?;

        trace!("receiving: pong");
        tokio::select! {
            msg = conn.next_message() => msg,
            _ = tokio::time::sleep(Duration::from_secs(5)) => Err(Error::PeerTimeout),
        }?;


        trace!("received: pong");

        debug!("Peer {:?} successfully sent a message on an encrypted channel", conn.uid);

        if !self.connected_peers.contains_key(&peer.uid) {
            self.connected_peers.insert(peer.uid.clone(), conn);
        } else {
            debug!("Deduplicating peer connection for {}", peer.uid);
        }

        Ok(())
    }

    pub fn connected_count(&self) -> usize {
        self.connected_peers.len()
    }

    pub fn missing_count(&self) -> usize {
        self.peers.len() - self.connected_peers.len()
    }

    pub fn health(&self) -> NetworkHealth {
        let connected = self.connected_count();
        let missing_peers = self.missing_count();

        match (connected, missing_peers) {
            (0, _) => NetworkHealth::Dead,
            (_, 0) => NetworkHealth::Perfect,
            _ => NetworkHealth::Degraded,
        }
    }
}

#[derive(Debug)]
pub struct FederatedNetworkBuilder {
    local: FederationPeer,
    peers: Vec<FederationPeer>,

    local_id_priv: Option<PathBuf>,
}

impl FederatedNetworkBuilder {
    pub fn new(local: FederationPeer, peers: Vec<FederationPeer>) -> FederatedNetworkBuilder {
        FederatedNetworkBuilder {
            local,
            peers,

            local_id_priv: None,
        }
    }

    // Path to the private key for the local identity
    pub fn with_local_key_path(mut self, path: Option<PathBuf>) -> FederatedNetworkBuilder {
        self.local_id_priv = path;
        self
    }

    // Generic Type M describes the user message type sent through the network
    pub async fn init<M>(self) -> crate::Result<Arc<FederatedNetwork<M>>> {
        let listener = self.init_server().await?;

        let peer_map: HashMap<String, FederationPeer> = self.peers
            .into_iter()
            .map(|p| (p.uid.clone(), p))
            .collect();

        let local    = Arc::new(self.local);
        let peers    = Arc::new(peer_map);
        let conn_map = Arc::new(DashMap::new());

        let auth_orchestrator = AuthOrchestrator::new(
            local.clone(),
            peers.clone(),
            conn_map.clone(),
            self.local_id_priv,
        );

        let network = FederatedNetwork {
            local,
            peers,

            connected_peers: conn_map,
            listener,

            seen_message_ids: HashMap::new(),

            auth_orchestrator,
        };

        Ok(Arc::new(network))
    }

    async fn init_server(&self) -> crate::Result<TcpListener> {
        let listener = TcpListener::bind(self.local.sock_addr()).await?;

        info!(
            "Initialized local federated endpoint at {}:{}", 
            self.local.address, 
            self.local.port
        );

        Ok(listener)
    }
}

pub struct PeerConnection<M> {
    uid: String,
    stream: Framed<TcpStream, EncryptionCodec<M>>,
    addr: SocketAddr,

    _message_type: PhantomData<M>,
}

impl<M> Debug for PeerConnection<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActivePeer")
            .field("uid", &self.uid)
            .field("stream", self.stream.get_ref())
            .field("addr", &self.addr)
            .finish()
    }
}

impl<M: PingPong + Serialize + DeserializeOwned> PeerConnection<M> {
    pub async fn new(
        auth_orchestrator: &AuthOrchestrator<M>,
        mut stream: TcpStream,
        addr: SocketAddr,
) -> crate::Result<PeerConnection<M>> {
        let (uid, cipher) = auth_orchestrator.authenticate(&mut stream).await?;

        let stream = Framed::new(stream, cipher.into());

        let peer = PeerConnection {
            uid,
            stream,
            addr,

            _message_type: PhantomData,
        };

        Ok(peer)
    }

    pub async fn connect<S: ToSocketAddrs>(
        auth_orchestrator: &AuthOrchestrator<M>,
        addr: S
    ) -> crate::Result<PeerConnection<M>> {
        let stream = TcpStream::connect(addr).await?;
        let addr = stream.peer_addr()?;

        PeerConnection::new(auth_orchestrator, stream, addr).await
    }

    pub async fn next_message(&mut self) -> crate::Result<PeerMessage<M>> {
        let result = self.next().await
            .ok_or(Error::PeerTimeout)?;

        match result {
            Err(FrameError::Decryption) => {
                Err(Error::PeerUnauthorised)
            },
            Err(e) => Err(e.into()),
            Ok(msg) => Ok(msg)
        }
    }
}


impl<M> Deref for PeerConnection<M> {
    type Target = Framed<TcpStream, EncryptionCodec<M>>;
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<M> DerefMut for PeerConnection<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NetworkHealth {
    Perfect,
    Degraded,
    Dead,
}

