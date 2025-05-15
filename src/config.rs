use std::path::PathBuf;

use crate::instance::GrafanaInstance;
use crate::Error;
use chrono::Duration;
use grafana_sync_federation::FederationPeer;
use log::debug;
use serde::Deserialize;
use serde_with::serde_as;
use serde_with::DurationSeconds;
use serde_with::OneOrMany;
use serde_with::formats::PreferMany;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Strict key mode is enabled but there are peers with unconfigured identity keys")]
    StrictKeyConfigIncomplete,
    #[error("Strict key mode is enabled but the local private key is not configured")]
    StrictKeyConfigLocalIncomplete,
    #[error("The file to a configured public key is missing")]
    PublicKeyFileMissing,
    #[error("The file to the local configured private key is missing")]
    PrivateKeyFileMissing,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FederationConfig {
    pub local_uid: String,
    pub instances: Vec<FederationPeer>,
    #[serde(default)]
    pub strict_key_mode: bool,
    pub local_private_key_file: Option<PathBuf>,
}

impl FederationConfig {
    pub fn local(&self) -> Option<&FederationPeer> {
        self.instances.iter().find(|i| i.uid == self.local_uid)
    }

    pub fn iter_peers(&self) -> impl Iterator<Item = &FederationPeer> {
        self.instances.iter().filter(|p| p.uid != self.local_uid)
    }

    pub fn check(&self) -> Result<(), ConfigError> {
        if let Some(key_path) = &self.local_private_key_file {
            if !key_path.exists() {
                return Err(ConfigError::PrivateKeyFileMissing);
            }
        } else if self.strict_key_mode {
            return Err(ConfigError::StrictKeyConfigLocalIncomplete);
        }

        for peer in self.iter_peers() {
            let Some(key_path) = &peer.public_key_file else {
                if self.strict_key_mode {
                    return Err(ConfigError::StrictKeyConfigIncomplete);
                }
                continue;
            };
            if !key_path.exists() {
                return Err(ConfigError::PublicKeyFileMissing);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrafanaConfig {
    pub instances: Vec<GrafanaInstance>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde_as(as = "OneOrMany<_, PreferMany>")]
    pub sync_tags: Vec<String>,
    #[serde_as(as = "DurationSeconds<i64>")]
    sync_rate_secs: Duration,
    pub federation: Option<FederationConfig>,
    pub grafana: GrafanaConfig,
}

impl Config {
    pub fn fetch(path: Option<&str>) -> crate::Result<Config> {
        let config: Config = config::Config::builder()
            .add_source(config::File::with_name(path.unwrap_or("config")))
            .build()?
            .try_deserialize()?;

        if config.sync_rate_secs.num_seconds() <= 0 {
            return Err(Error::SyncIntervalOutOfRange);
        }

        Ok(config)
    }

    pub(crate) fn dbg_print(&self) {
        debug!("Full configuration:");

        debug!("  + Sync Tags: {:?}",   self.sync_tags);
        debug!("  + Sync Rate: {}s", self.sync_rate_secs);

        debug!("");

        for (i, instance) in self.grafana.instances.iter().enumerate() {
            debug!("  + Grafana Instance: #{i}:");
            debug!("    - URL: {}",   instance.base_url());
        }

        debug!("");

        if let Some(federation) = &self.federation {
            if federation.strict_key_mode {
                debug!("  + !! Strict Key Mode activated !!")
            }
            debug!("  + Local Federation ID: {}", federation.local_uid);
            if let Some(key) = &federation.local_private_key_file {
                debug!("  + Local Federation Private Key File: {}", key.display());
            }

            for (i, peer) in federation.iter_peers().enumerate() {
                debug!("  + Federation Peer: #{i}:");
                debug!("    - UID: {}",     peer.uid);
                debug!("    - Address: {}", peer.address);
                debug!("    - Port: {}",    peer.port);
                if let Some(key) = &peer.public_key_file {
                    debug!("    - Public Key File: {}", key.display());
                }
            }
        }
    }

    pub fn sync_rate(&self) -> &Duration {
        &self.sync_rate_secs
    }
}
