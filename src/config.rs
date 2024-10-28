use std::{fs, io};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use log::{info, warn};
use serde_yaml::Value;
use tracing::instrument;
use crate::error::GSError;
use crate::instance::GrafanaInstance;

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    instance_master: GrafanaInstance,
    instance_slaves: Vec<GrafanaInstance>,
    dashboard: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    service: ServiceConfig,
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

    fn get_yaml_path<'a>(config: &'a Value, key: &str) -> Result<&'a Value, GSError> {
        let keys = key.split('.');
        let mut value = config;

        for key in keys {
           value = value.get(key)
                .ok_or_else(|| GSError::ConfigKeyMissing(key.to_string()))?;
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
    fn collect_slaves(config: &Value) -> Result<Vec<GrafanaInstance>, GSError> {
        let mut slaves = Vec::new();

        let instance_slaves = Self::get_yaml_path(config, "service.instanceSlaves");

        let instance_slaves = match instance_slaves {
            Err(_) => {
                warn!("No slaves are defined.");
                return Ok(slaves)
            },
            Ok(slaves) => slaves,
        };

        let instance_slaves = instance_slaves.as_sequence()
            .ok_or(GSError::ConfigKeyTypeWrong("service.instanceSlaves".to_string(), "Sequence"))?;

        for (i, instance_slave) in instance_slaves.iter().enumerate() {
            let key = format!("service.instanceSlaves[{}].url", i);
            let url = instance_slave.get("url")
                .ok_or_else(|| GSError::ConfigKeyMissing(key.clone()))?
                .as_str()
                .ok_or_else(|| GSError::ConfigKeyTypeWrong(key.clone(), "String"))?
                .to_string();

            let key = format!("service.instanceSlaves[{}].api_token", i);
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
            info!("  - {}", slave.url());
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

        let url = Self::read_string_from_config(&config, "service.instanceMaster.url")?;
        let api_token = Self::read_string_from_config(&config, "service.instanceMaster.api_token")?.into();

        let instance_master = GrafanaInstance::new(url, api_token);
        let instance_slaves = Self::collect_slaves(&config)?;
        let dashboards = Self::collect_dashboards(&config)?;

        Ok(Config {
            service: ServiceConfig {
                instance_master,
                instance_slaves: instance_slaves,
                dashboard: dashboards,
            },
        })
    }

    pub(crate) fn dbg_print(&self) {
        println!("Full configuration:");
        println!("  - Service configuration:");
        println!("    - Instance master:");
        println!("      - URL: {}", self.service.instance_master.url());
        println!("      - Token: {}", self.service.instance_master.api_token().value());
        println!("    - Service slaves:");

        for (i, slave) in self.service.instance_slaves.iter().enumerate() {
            println!("      + Slave #{i}:");
            println!("        - URL: {}", slave.url());
            println!("        - Token: {}", slave.api_token().value());
        }

        println!("    - Synced Dashboards:");
        for (i, board) in self.service.dashboard.iter().enumerate() {
            println!("      + Dashboard #{i}:");
            println!("        - Path: {}", board);
        }
    }
}