use std::path::PathBuf;
use ed25519_dalek::{pkcs8::DecodePublicKey, VerifyingKey};
use zeroize::Zeroizing;

pub type PeerUID = String;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PeerInfo {
    pub uid: PeerUID,
    pub address: String,
    pub port: u16,
}

fn default_port() -> u16 { 8880 }

#[derive(Clone, serde::Deserialize)]
pub struct FederationPeer {
    pub uid: PeerUID,
    pub address: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub token: Zeroizing<String>,

    // the expected identity pubkey, to protect against MITM
    pub public_key_file: Option<PathBuf>,
}

impl FederationPeer {
    pub fn info(&self) -> PeerInfo {
        PeerInfo {
            uid: self.uid.clone(),
            address: self.address.clone(),
            port: self.port,
        }
    }

    pub fn sock_addr(&self) -> (&str, u16) {
        (self.address.as_str(), self.port)
    }

    // loads the peers identity public key from the provided path on disk
    pub fn load_id_pub(&self) -> crate::Result<Option<VerifyingKey>> {
        let Some(pub_path) = self.public_key_file.as_ref() else {
            return Ok(None);
        };

        let key = match pub_path.extension().and_then(|e| e.to_str()) {
            Some("der") => VerifyingKey::read_public_key_der_file(pub_path)?,
            _ => VerifyingKey::read_public_key_pem_file(pub_path)?,
        };

        Ok(Some(key))
    }
}

