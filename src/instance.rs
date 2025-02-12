use crate::encrypted_cred::EncryptedCredential;
use crate::error::GSError;
use reqwest::header::{HeaderMap, HeaderValue};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct GrafanaInstance {
    url: String,
    api_token: EncryptedCredential,
    is_master: bool,
    sync_tag: Option<String>,
    http_client: reqwest::Client,
}

impl GrafanaInstance {
    fn _make_new_client(api_token: &EncryptedCredential) -> Result<reqwest::Client, GSError> {
        let mut header_map = HeaderMap::new();
        header_map.insert(
            "Authorization",
            HeaderValue::try_from(format!("Bearer {}", api_token.value()))?,
        );
        header_map.insert("accept", HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(header_map)
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;

        Ok(client)
    }

    pub fn new(url: String, api_token: EncryptedCredential) -> Result<Self, GSError> {
        let http_client = Self::_make_new_client(&api_token)?;
        Ok(GrafanaInstance {
            url,
            api_token,
            is_master: false,
            sync_tag: None,
            http_client,
        })
    }

    pub fn base_url(&self) -> &str {
        self.url.as_str()
    }

    pub fn api_token(&self) -> &EncryptedCredential {
        &self.api_token
    }

    pub fn make_master(&mut self, sync_tag: String) {
        self.is_master = true;
        self.sync_tag = Some(sync_tag);
    }

    #[instrument]
    pub fn client(&self) -> &reqwest::Client {
        &self.http_client
    }

    pub fn sync_tag(&mut self) -> Option<&str> {
        Some(self.sync_tag.as_ref()?.as_str())
    }
}
