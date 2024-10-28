use log::{error, info};
use crate::config::Config;

mod service;
mod encrypted_cred;
mod config;
mod error;
mod instance;

#[tokio::main]
async fn main() {
    match run().await {
        Err(e) => error!("GraphSync exited with error: {}", e),
        _ => info!("Exiting."),
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config = Config::use_config_file("config.yaml")?;

    config.dbg_print();

    Ok(())
}
