use crate::config::Config;
use crate::error::GSError;
use crate::service::SyncService;
use std::env;
use tracing::instrument;
use tracing::{error, info};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;

pub mod api;
mod config;
mod dashboard_state;
mod encrypted_cred;
mod error;
mod instance;
mod service;

#[tokio::main]
async fn main() {
    match run().await {
        Err(e) => error!("Grafana Sync exited with error: {}", e),
        _ => info!("Exiting."),
    }
}

#[instrument]
async fn run() -> Result<(), GSError> {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_target(false)
        .init();

    let args: Vec<String> = env::args().collect();
    let config_path = args.get(1).map(|str| str.as_str()).unwrap_or("config.yaml");

    let config = Config::use_config_file(config_path)?;

    config.dbg_print();

    SyncService::new(config).run().await?;

    Ok(())
}
