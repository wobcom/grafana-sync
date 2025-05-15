use std::io;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Encode(#[from] bincode::error::EncodeError),
    #[error(transparent)]
    Decode(#[from] bincode::error::DecodeError),
    #[error("Local key was invalid (use .pem or .der private key as path)")]
    KeyPathInvalid,
    #[error("Local key was not found")]
    KeyPathNotFound,
    #[error(transparent)]
    Pkcs8(#[from] ed25519_dalek::pkcs8::Error),
    #[error(transparent)]
    Spki(#[from] ed25519_dalek::pkcs8::spki::Error),
    #[error(transparent)]
    PubKeyFormat(#[from] ed25519_dalek::ed25519::Error),

    #[error("Peer was accepted but token was not found")]
    PeerTokenNotFound,
    #[error("Peer response timed out")]
    PeerTimeout,
    #[error("Peer is not compatible with this version")]
    PeerIncompatible,
    #[error("Peer sent an invalid message")]
    PeerMessageUnexpected,
    #[error("Peer pubkey was denied as it didn't match the supplied key")]
    PeerPubkeyInvalid,
    #[error("Peer is unauthorised to connect")]
    PeerUnauthorised,

    #[error("The given key had an invalid length for the HKDF")]
    HkdfInvalidLength,
    #[error(transparent)]
    Frame(#[from] FrameError),

    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Encode(#[from] bincode::error::EncodeError),
    #[error(transparent)]
    Decode(#[from] bincode::error::DecodeError),
    #[error("Encryption error")]
    Encryption,
    #[error("Decryption error")]
    Decryption,
    #[error("The buffer got too big for a single frame")]
    BufTooBig(#[from] std::num::TryFromIntError),
    #[error("The received frame was suspiciously malformed. This might be a coincidence.")]
    PossiblyMaliciousFrame,
    #[error("The requested frame size exceeded the maximum size")]
    FrameTooBig,
}
