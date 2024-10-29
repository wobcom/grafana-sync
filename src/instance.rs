use reqwest::header::{HeaderMap, HeaderValue};
use tracing::instrument;
use crate::encrypted_cred::EncryptedCredential;
use crate::error::GSError;

#[derive(Debug, Clone)]
pub struct GrafanaInstance {
    url: String,
    api_token: EncryptedCredential,
    is_master: bool,
    sync_tag: Option<String>,
    running_requester: Option<reqwest::Client>,
}

impl GrafanaInstance {
    pub fn base_url(&self) -> &str {
        self.url.as_str()
    }

    pub fn api_token(&self) -> &EncryptedCredential {
        &self.api_token
    }

    pub fn new(url: String, api_token: EncryptedCredential) -> Self {
        GrafanaInstance {
            url,
            api_token,
            is_master: false,
            sync_tag: None,
            running_requester: None,
        }
    }

    pub fn make_master(&mut self, sync_tag: String) {
        self.is_master = true;
        self.sync_tag = Some(sync_tag);
    }

    #[instrument]
    pub fn client(&mut self) -> Result<&reqwest::Client, GSError> {
        if self.running_requester.is_none() {
            let mut header_map = HeaderMap::new();
            header_map.insert("Authorization", HeaderValue::try_from(format!("Bearer {}", self.api_token.value()))?);
            header_map.insert("accept", HeaderValue::from_static("application/json"));

            let client = reqwest::Client::builder()
                .default_headers(header_map)
                .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
                .build()?;

            self.running_requester = Some(client);
        }

        Ok(self.running_requester.as_ref().unwrap())
    }

    pub fn sync_tag(&mut self) -> Option<&str> {
        Some(self.sync_tag.as_ref()?.as_str())
    }
}