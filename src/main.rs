#![feature(duration_constructors)]

use std::env;
use log::{error, info, LevelFilter};
use tracing::instrument;
use crate::config::Config;
use crate::error::GSError;
use crate::service::SyncService;

mod service;
mod encrypted_cred;
mod config;
mod error;
mod instance;
pub mod api;
mod dashboard_state;

#[tokio::main]
async fn main() {
    match run().await {
        Err(e) => error!("GraphSync exited with error: {}", e),
        _ => info!("Exiting."),
    }
}

#[instrument]
async fn run() -> Result<(), GSError> {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .format_target(false)
        .parse_default_env()
        .init();

    let args: Vec<String> = env::args().collect();
    let config_path = args.get(1)
        .map(|str| str.as_str())
        .unwrap_or("config.yaml");

    let config = Config::use_config_file(config_path)?;

    config.dbg_print();

    SyncService::new(config).run().await?;

    Ok(())
}

#[test]
fn create_1000_dashboards_on_master() {
    use api::dashboards::Folder;
    
    let args: Vec<String> = env::args().collect();
    let config_path = args.get(1)
        .map(|str| str.as_str())
        .unwrap_or("config.yaml");

    let config = Config::use_config_file(config_path).unwrap();

    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut dash = runtime.block_on(config.instances[0].get_dashboard_full("ee2blhu68eqyod")).unwrap();
    let folder = Folder {
        id: dash.meta.folder_id as u32,
        uid: dash.meta.folder_uid.clone(),
        title: dash.meta.folder_title.clone(),
    };

    for i in 0..1000 {
        dash.dashboard.title = format!("MeowBoard {i} :3");
        runtime.block_on(config.instances[0].import_dashboard(&dash, &folder, false)).unwrap();
    }
}