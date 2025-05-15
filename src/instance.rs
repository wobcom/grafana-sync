use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct GrafanaInstance {
    url: String,
    http_client: reqwest::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrafanaInfo {
    url: String,
}

impl<'de> Deserialize<'de> for GrafanaInstance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {
            #[derive(Deserialize)]
            struct RawGrafanaInstance {
                url: String,
                api_token: String,
            }

            let raw = RawGrafanaInstance::deserialize(deserializer)?;
            let instance = GrafanaInstance::new(raw.url, raw.api_token)
                .map_err(serde::de::Error::custom)?;

            Ok(instance)
    }
}

impl GrafanaInstance {
    fn _make_new_client(api_token: &str) -> crate::Result<reqwest::Client> {
        let mut header_map = HeaderMap::new();
        header_map.insert(
            "Authorization",
            HeaderValue::try_from(format!("Bearer {}", api_token))?,
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

    pub fn new(url: String, api_token: String) -> crate::Result<Self> {
        let http_client = Self::_make_new_client(&api_token)?;
        Ok(GrafanaInstance {
            url,
            http_client,
        })
    }

    pub fn base_url(&self) -> &str {
        self.url.as_str()
    }

    #[instrument]
    pub fn client(&self) -> &reqwest::Client {
        &self.http_client
    }
}
