use std::{fs, io};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use log::{debug, error, info, warn};
use serde_yaml::Value;
use tracing::instrument;
use crate::api::dashboards::{Folder, SimpleDashboard};
use crate::error::GSError;
use crate::instance::GrafanaInstance;

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub instance_master: GrafanaInstance,
    pub instance_slaves: Vec<GrafanaInstance>,
    pub dashboard: Vec<String>,
    pub sync_rate_mins: u64,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub service: ServiceConfig,
}

impl Config {
    fn get_or_create<P: AsRef<Path>>(path: P) -> io::Result<File> {
        if !fs::exists(&path)? {
            info!("No config file exists yet. Creating one at {}", path.as_ref().display());

            let mut file = File::create_new(&path)?;

            file.write_all(include_bytes!("default/config.yaml"))?;
        }

        File::open(&path)
    }

    fn get_yaml_path<'a>(config: &'a Value, full_key: &str) -> Result<&'a Value, GSError> {
        let keys = full_key.split('.');
        let mut value = config;

        for key in keys {
           value = value.get(key)
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
    fn collect_slaves(config: &Value) -> Result<Vec<GrafanaInstance>, GSError> {
        let mut slaves = Vec::new();

        let instance_slaves = Self::get_yaml_path(config, "service.instance_slaves");

        let instance_slaves = match instance_slaves {
            Err(_) => {
                warn!("No slaves are defined.");
                return Ok(slaves)
            },
            Ok(slaves) => slaves,
        };

        let instance_slaves = instance_slaves.as_sequence()
            .ok_or(GSError::ConfigKeyTypeWrong("service.instance_slaves".to_string(), "Sequence"))?;

        for (i, instance_slave) in instance_slaves.iter().enumerate() {
            let key = format!("service.instance_slaves[{}].url", i);
            let url = instance_slave.get("url")
                .ok_or_else(|| GSError::ConfigKeyMissing(key.clone()))?
                .as_str()
                .ok_or_else(|| GSError::ConfigKeyTypeWrong(key.clone(), "String"))?
                .to_string();

            let key = format!("service.instance_slaves[{}].api_token", i);
            let api_token= instance_slave.get("api_token")
                .ok_or_else(|| GSError::ConfigKeyMissing(key.clone()))?
                .as_str()
                .ok_or_else(|| GSError::ConfigKeyTypeWrong(key.clone(), "String"))?
                .to_string()
                .into();

            slaves.push(GrafanaInstance::new(url, api_token));
        }

        info!("Loaded {} slaves:", instance_slaves.len());
        for slave in &slaves {
            info!("  - {}", slave.base_url());
        }

        Ok(slaves)
    }

    fn collect_dashboards(config: &Value) -> Result<Vec<String>, GSError> {
        let mut dashboards = Vec::new();

        let dashboard_seq = Self::get_yaml_path(config, "service.dashboards");

        let dashboard_seq = match dashboard_seq {
            Err(_) => {
                warn!("No dashboard is defined.");
                return Ok(dashboards);
            }
            Ok(dashboard_seq) => dashboard_seq,
        };

        let dashboard_seq = dashboard_seq.as_sequence()
            .ok_or_else(|| GSError::ConfigKeyTypeWrong("service.dashboards".to_string(), "Sequence"))?;

        for (i, dashboard) in dashboard_seq.iter().enumerate() {
            let key = format!("service.dashboards[{}].url", i);
            let location = match dashboard.as_str() {
                None => {
                    warn!("Dashboard path at key \"{}\" is not a valid string. Use something like \"Folder/Dashboard\".", key);
                    continue;
                },
                Some(loc) => loc,
            };

            dashboards.push(location.to_string());
        }

        info!("Registered {} dashboards for syncing.", dashboards.len());

        Ok(dashboards)
    }

    pub fn use_config_file<P: AsRef<Path>>(path: P) -> Result<Config, GSError> {
        let file = Self::get_or_create(&path)?;

        let config = serde_yaml::from_reader::<_, Value>(file)?;

        let url = Self::read_string_from_config(&config, "service.instance_master.url")?;
        let api_token = Self::read_string_from_config(&config, "service.instance_master.api_token")?.into();
        let sync_tag = Self::read_string_from_config(&config, "service.instance_master.sync_tag")?;
        let sync_rate_mins = Self::read_u64_from_config(&config, "service.sync_rate_mins")?;

        let mut instance_master = GrafanaInstance::new(url, api_token);
        instance_master.make_master(sync_tag);
        info!("Loaded master \"{}\"", instance_master.base_url());

        let instance_slaves = Self::collect_slaves(&config)?;
        let dashboards = Self::collect_dashboards(&config)?;

        Ok(Config {
            service: ServiceConfig {
                instance_master,
                instance_slaves,
                dashboard: dashboards,
                sync_rate_mins,
            },
        })
    }

    pub(crate) fn dbg_print(&self) {
        debug!("Full configuration:");
        debug!("  - Service configuration:");
        debug!("    - Instance master:");
        debug!("      - URL: {}", self.service.instance_master.base_url());
        debug!("      - Token: {}", self.service.instance_master.api_token().value());
        debug!("    - Service slaves:");

        for (i, slave) in self.service.instance_slaves.iter().enumerate() {
            debug!("      + Slave #{i}:");
            debug!("        - URL: {}", slave.base_url());
            debug!("        - Token: {}", slave.api_token().value());
        }

        debug!("    - Synced Dashboards:");
        for (i, board) in self.service.dashboard.iter().enumerate() {
            debug!("      + Dashboard #{i}:");
            debug!("        - Path: {}", board);
        }
    }
}

impl ServiceConfig {
    pub async fn replicate_to_slaves(&mut self, dashboard: &SimpleDashboard) -> Result<(), GSError> {
        let mut slave_folders: Vec<(&mut GrafanaInstance, Folder)> = Vec::new();
        for slave in &mut self.instance_slaves {
            match slave.ensure_folder(&dashboard.folder_title).await {
                Ok(folder) => {
                    slave_folders.push((slave, folder));
                }
                Err(e) => {
                    error!("Couldn't sync folder for instance {}. Error: {e}", slave.base_url());
                }
            }
        }

        let mut full_dashboard = self.instance_master.get_dashboard_full(&dashboard.uid).await?;
        full_dashboard.sanitize();
        for (slave, folder) in slave_folders {
            info!("Starting replication of \"{}\" onto {}", dashboard.title, slave.base_url());
            slave.import_dashboard(&full_dashboard, &folder, true).await?;
        }

        Ok(())
    }
}