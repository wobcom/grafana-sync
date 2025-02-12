use aes_gcm::aead::consts::U12;
use aes_gcm::aead::{Aead, Nonce, OsRng};
use aes_gcm::aes::Aes256;
use aes_gcm::{AeadCore, Aes256Gcm, AesGcm, KeyInit};
use std::fmt::{Debug, Formatter};
use tracing::instrument;

#[derive(Clone)]
pub struct EncryptedCredential {
    cred: Vec<u8>,
    cipher: Aes256Gcm,
    nonce: Nonce<AesGcm<Aes256, U12>>,
}

// Because of the guarantee that a String is valid UTF-8, except is used since other cases would be UB
impl EncryptedCredential {
    #[instrument]
    pub fn new(credential: String) -> EncryptedCredential {
        let key = Aes256Gcm::generate_key(OsRng);
        let cipher = Aes256Gcm::new(&key);
        let nonce = Aes256Gcm::generate_nonce(OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, credential.as_bytes())
            .expect("encryption failure!");

        EncryptedCredential {
            cred: ciphertext,
            cipher,
            nonce,
        }
    }

    #[instrument]
    pub fn value(&self) -> String {
        let plaintext = self
            .cipher
            .decrypt(&self.nonce, self.cred.as_slice())
            .expect("decryption failure!");
        let string = String::from_utf8(plaintext).expect("decryption failure!");

        string
    }
}

impl Debug for EncryptedCredential {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.cred)
    }
}

impl From<String> for EncryptedCredential {
    #[instrument]
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
