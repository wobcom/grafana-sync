#![feature(duration_constructors)]

use log::{error, info};
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

#[tokio::main]
async fn main() {
    match run().await {
        Err(e) => error!("GraphSync exited with error: {}", e),
        _ => info!("Exiting."),
    }
}

#[instrument]
async fn run() -> Result<(), GSError> {
    env_logger::init();

    let config = Config::use_config_file("config.yaml")?;

    config.dbg_print();

    SyncService::new(config).run().await?;

    Ok(())
}
