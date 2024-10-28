use crate::encrypted_cred::EncryptedCredential;

#[derive(Debug, Clone)]
pub struct GrafanaInstance {
    url: String,
    api_token: EncryptedCredential,
}

impl GrafanaInstance {
    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    pub fn api_token(&self) -> &EncryptedCredential {
        &self.api_token
    }

    pub fn new(url: String, api_token: EncryptedCredential) -> Self {
        GrafanaInstance {
            url,
            api_token,
        }
    }
}