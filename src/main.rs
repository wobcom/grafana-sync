#![feature(duration_constructors)]

use std::env;
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

    let args: Vec<String> = env::args().collect();
    let config_path = args.get(1)
        .map(|str| str.as_str())
        .unwrap_or("config.yaml");

    let config = Config::use_config_file(config_path)?;

    config.dbg_print();

    SyncService::new(config).run().await?;

    Ok(())
}
