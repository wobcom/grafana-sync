use crate::config::Config;
use crate::service::SyncService;
use log::{error, info, LevelFilter};
use std::env;
use tracing::instrument;

pub mod api;
pub mod error;
mod config;
mod dashboard_state;
mod instance;
mod service;
mod federation;

pub use error::*;

#[tokio::main]
async fn main() {
    match run().await {
        Err(e) => error!("Grafana Sync exited with error: {}", e),
        _ => info!("Exiting."),
    }
}

#[instrument]
async fn run() -> crate::Result<()> {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .format_target(false)
        .parse_default_env()
        .init();

    let args: Vec<String> = env::args().collect();
    let config_path = args.get(1).map(|s| s.as_str());

    let config = Config::fetch(config_path)?;

    config.dbg_print();

    SyncService::new(config).run().await?;

    Ok(())
}
