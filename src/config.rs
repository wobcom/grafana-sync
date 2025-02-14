use crate::error::GSError;
use crate::instance::GrafanaInstance;
use log::{debug, info, warn};
use serde_yaml::Value;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::{fs, io};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct Config {
    pub instances: Vec<GrafanaInstance>,
    pub sync_tag: String,
    pub sync_rate_mins: u64,
}

impl Config {
    fn get_or_create<P: AsRef<Path>>(path: P) -> io::Result<File> {
        if !fs::exists(&path)? {
            info!(
                "No config file exists yet. Creating one at {}",
                path.as_ref().display()
            );

            let mut file = File::create_new(&path)?;

            file.write_all(include_bytes!("default/config.yaml"))?;
        }

        File::open(&path)
    }

    fn get_yaml_path<'a>(config: &'a Value, full_key: &str) -> Result<&'a Value, GSError> {
        let keys = full_key.split('.');
        let mut value = config;

        for key in keys {
            value = value
                .get(key)
                .ok_or_else(|| GSError::ConfigKeyMissing(full_key.to_string()))?;
        }

        Ok(value)
    }

    #[instrument]
    fn read_string_from_config(config: &Value, key: &str) -> Result<String, GSError> {
        let value = Self::get_yaml_path(config, key)?;

        Ok(value
            .as_str()
            .ok_or_else(|| GSError::ConfigKeyTypeWrong(key.to_string(), "String"))?
            .to_string())
    }

    #[instrument]
    fn read_u64_from_config(config: &Value, key: &str) -> Result<u64, GSError> {
        let value = Self::get_yaml_path(config, key)?;

        Ok(value
            .as_u64()
            .ok_or_else(|| GSError::ConfigKeyTypeWrong(key.to_string(), "u64"))?)
    }

    #[instrument]
    fn collect_instances(config: &Value) -> Result<Vec<GrafanaInstance>, GSError> {
        let mut instances = Vec::new();

        let cfg_instances = Self::get_yaml_path(config, "instances");

        let json_instances = match cfg_instances {
            Err(_) => {
                warn!("No instances are defined.");
                return Ok(instances);
            }
            Ok(instances) => instances,
        };

        let json_instances = json_instances
            .as_sequence()
            .ok_or(GSError::ConfigKeyTypeWrong(
                "instances".to_string(),
                "Sequence",
            ))?;

        for (i, instance) in json_instances.iter().enumerate() {
            let key = format!("instances[{}].url", i);
            let url = instance
                .get("url")
                .ok_or_else(|| GSError::ConfigKeyMissing(key.clone()))?
                .as_str()
                .ok_or_else(|| GSError::ConfigKeyTypeWrong(key.clone(), "String"))?
                .to_string();

            let key = format!("instances[{}].api_token", i);
            let api_token = instance
                .get("api_token")
                .ok_or_else(|| GSError::ConfigKeyMissing(key.clone()))?
                .as_str()
                .ok_or_else(|| GSError::ConfigKeyTypeWrong(key.clone(), "String"))?
                .to_string()
                .into();

            instances.push(GrafanaInstance::new(url, api_token)?);
        }

        info!("Loaded {} instance(s):", json_instances.len());
        for instance in &instances {
            info!("  - {}", instance.base_url());
        }

        Ok(instances)
    }

    pub fn use_config_file<P: AsRef<Path>>(path: P) -> Result<Config, GSError> {
        let file = Self::get_or_create(&path)?;

        let config = serde_yaml::from_reader::<_, Value>(file)?;

        let sync_tag = Self::read_string_from_config(&config, "sync_tag")?;
        let sync_rate_mins = Self::read_u64_from_config(&config, "sync_rate_mins")?;

        let instances = Self::collect_instances(&config)?;

        Ok(Config {
            sync_tag,
            instances,
            sync_rate_mins,
        })
    }

    pub(crate) fn dbg_print(&self) {
        debug!("Full configuration:");

        debug!("  + Sync Tag: {}", self.sync_tag);
        debug!("  + Sync Rate: {}", self.sync_rate_mins);
        for (i, instance) in self.instances.iter().enumerate() {
            debug!("  + Instance: #{i}:");
            debug!("    - URL: {}", instance.base_url());
            debug!("    - Token: {}", instance.api_token().checkable_obfuscated());
        }
    }
}
