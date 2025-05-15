use std::{collections::HashMap, path::PathBuf, sync::Arc, time::{Duration, Instant}};

use async_bincode::tokio::AsyncBincodeStream;
use bincode::error::{DecodeError, EncodeError};
use chacha20poly1305::{aead::OsRng, Key, KeyInit, XChaCha20Poly1305};
use dashmap::DashMap;
use ed25519_dalek::{pkcs8::DecodePrivateKey, SigningKey, VerifyingKey};
use hkdf::Hkdf;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::net::TcpStream;
use futures::{Sink, SinkExt, Stream, StreamExt};
use x25519_dalek::{PublicKey, StaticSecret};
use serde_big_array::BigArray;
use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::{Error, FederationPeer, PeerConnection, PeerMessage, PingPong, VERSION};

#[derive(Debug)]
pub struct AuthOrchestrator<M> {
    local: Arc<FederationPeer>,
    peers: Arc<HashMap<String, FederationPeer>>,
    connected_peers: Arc<DashMap<String, PeerConnection<M>>>,

    id_prv_path: Option<PathBuf>,
    generated_id_key: SigningKey,
}

impl<M> AuthOrchestrator<M> {
    pub fn new(
        local: Arc<FederationPeer>,
        peers: Arc<HashMap<String, FederationPeer>>,
        connected_peers: Arc<DashMap<String, PeerConnection<M>>>,
        id_prv_path: Option<PathBuf>,
    ) -> Self
    {
        if id_prv_path.is_some() {
            info!("Auth orchestration will use the supplied local public key for identification");
        }

        AuthOrchestrator {
            local,
            peers,
            connected_peers,

            id_prv_path,
            generated_id_key: SigningKey::generate(&mut OsRng),
        }
    }

    pub fn ident_pub(&self) -> crate::Result<VerifyingKey> {
        if self.id_prv_path.is_some() {
            Ok(self.ident_pub_from_file()?)
        } else {
            Ok(self.ident_pub_generated())
        }
    }

    pub fn ident_pub_from_file(&self) -> crate::Result<VerifyingKey> {
        let pub_path = self.id_prv_path.as_ref().ok_or(Error::KeyPathNotFound)?;

        let key = match pub_path.extension().and_then(|e| e.to_str()) {
            Some("der") => SigningKey::read_pkcs8_der_file(pub_path)?,
            _ => SigningKey::read_pkcs8_pem_file(pub_path)?,
        };

        Ok(key.verifying_key())
    }

    pub fn ident_pub_generated(&self) -> VerifyingKey {
        self.generated_id_key.verifying_key()
    }

    fn local_uid(&self) -> &str {
        &self.local.uid
    }

    fn peer_psk(&self, uid: &str) -> Option<&str> {
        self.peers.get(uid).map(|p| p.token.as_str())
    }
}

#[derive(Serialize, Deserialize, ZeroizeOnDrop)]
enum InitMsgType {
    Hello {
        version: u64,
        uid: String,
        ident_pub: [u8; 32],
        eph_pub: [u8; 32],
    },
    VersionUnsupported,
    PeerUnknown,
    PubkeyInvalid, // only in strict mode

    Connect {
        timestamp: u64,
        #[serde(with = "BigArray")]
        signature: [u8; 64],
        psk_salt: [u8; 32],
        psk_hash: [u8; 32],
    },
}

impl PingPong for InitMsgType {
    fn ping() -> Self {
        unreachable!()
    }

    fn pong() -> Self {
        unreachable!()
    }
}

impl InitMsgType {
    // returns the hello message and the used session-bound ephemeral secret key
    fn produce_hello<M>(auth: &AuthOrchestrator<M>) -> crate::Result<(Zeroizing<StaticSecret>, PeerMessage<InitMsgType>)> {
        let ident_pub = auth.ident_pub()?;
        let eph_key = Zeroizing::new(StaticSecret::random_from_rng(OsRng));
        let eph_pub = x25519_dalek::PublicKey::from(&*eph_key); // just saying: won't zeroize
        let hello = InitMsgType::Hello {
            version: VERSION,
            uid: auth.local_uid().to_string(),
            ident_pub: ident_pub.to_bytes(),
            eph_pub: eph_pub.to_bytes(),
        }.into();

        Ok((eph_key, hello))
    }
}

type InitMessage = PeerMessage<InitMsgType>;

impl<M> AuthOrchestrator<M> {
    pub async fn authenticate(
        &self,
        stream: &mut TcpStream, 
                        // uid, cipher
    ) -> crate::Result<(String, XChaCha20Poly1305)> {
        let mut stream = AsyncBincodeStream::from(stream).for_async();

        let (local_eph_key, hello) = InitMsgType::produce_hello(self)?;
        stream.send(hello).await?;

        if let Ok(addr) = stream.get_ref().peer_addr() {
            debug!("Sent HELLO to {:?}", addr);
        }
        
        let (_remote_id_pub, remote_eph_pub, uid) = self.authorization_msg_loop(&mut stream).await?;

        // drop the cleartext stream wrapper since the stream should be sending encrypted now
        drop(stream);

        let remote_eph_pub = PublicKey::from(remote_eph_pub);
        let shared_secret = local_eph_key.diffie_hellman(&remote_eph_pub);

        debug!(
            "Created new shared secret for communication with {uid:?}. Checksum: {}", 
            shared_secret.as_bytes().iter().fold(0i64, |acc, b| acc + *b as i64)
        );

        let peer_psk = self.peer_psk(&uid)
            .ok_or(Error::PeerTokenNotFound)?;

        let hk = Hkdf::<Sha256>::new(None, &[shared_secret.as_bytes(), peer_psk.as_bytes()].concat());
        let mut session_key = [0u8; 32];
        hk.expand(b"meow session key :3", &mut session_key)
            .map_err(|_| Error::HkdfInvalidLength)?;

        let cipher = XChaCha20Poly1305::new(Key::from_slice(&session_key));

        debug!("Establishing encrypted stream with XChaCha20Poly1305 cipher...");

        Ok((uid, cipher))
    }

    // establish stream encryption and authenticate client
    // returns the pubkey of the authenticating client for encryption
    // the msg passing will be encrypted hereafter
    async fn authorization_msg_loop<S>(&self, stream: &mut S) -> crate::Result<([u8; 32], [u8; 32], String)>
    where
        S: Stream<Item = Result<InitMessage, DecodeError>> 
            + Sink<InitMessage, Error = EncodeError> 
            + Unpin
    {
        let start = Instant::now();
        const TIMEOUT: Duration = Duration::from_secs(10);
        const LOOP_DELAY: Duration = Duration::from_millis(100);

        loop {
            if start.elapsed() > TIMEOUT {
                return Err(Error::PeerTimeout);
            }
            
            let response: InitMessage = tokio::select! {
                Some(response) = stream.next() => response?,
                _ = tokio::time::sleep(LOOP_DELAY) => continue,
            };

            let Some(msg) = response.into_msg() else {
                continue;
            };

            return self.handle_initial_hello(stream, msg).await;
        }
    }

    async fn handle_initial_hello<S>(
        &self,
        stream: &mut S, 
        msg: InitMsgType,
    ) -> crate::Result<([u8; 32], [u8; 32], String)>
    where
        S: Sink<InitMessage, Error = EncodeError> + Unpin
    {
        // TODO: Handle pubkey verification
        match &msg {
            InitMsgType::Hello { version, uid, ident_pub, eph_pub } => {
                if *version != VERSION {
                    info!("Disconnected client {uid:?} for using an incompatible version");
                    stream.send(InitMsgType::VersionUnsupported.into()).await?;
                    return Err(Error::PeerIncompatible);
                }

                let Some(peer) = self.peers.get(uid) else {
                    info!("Disconnected client {uid:?} because they do not appear in the configured peer list");
                    stream.send(InitMsgType::PeerUnknown.into()).await?;
                    return Err(Error::PeerIncompatible);
                };

                // TODO: Actually verify that the peer has ownership of this public key
                //   Currently this can just be spoofed as no authority is checked
                if let Some(expected_pub) = peer.load_id_pub()? {
                    let given_pub = VerifyingKey::from_bytes(ident_pub)?;
                    
                    if expected_pub != given_pub {
                        info!("Disconnected client {uid:?} because their public key didn't match the configured one");
                        return Err(Error::PeerPubkeyInvalid)
                    }
                }

                debug!("Connecting client turned out to be known peer {uid:?}");

                debug!("Known peer sent a valid HELLO packet");

                Ok((*ident_pub, *eph_pub, uid.clone()))
            },
            _ => Err(Error::PeerMessageUnexpected),
        }
    }

}
