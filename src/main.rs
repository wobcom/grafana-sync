use log::{debug, error, info};
use tracing::instrument;
use crate::config::Config;

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
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut config = Config::use_config_file("config.yaml")?;

    config.dbg_print();

    let tags=  config.service.instance_master.get_tags().await?;
    debug!("Master Tags: {tags:?}");

    let sync_tag = config.service.instance_master.sync_tag().unwrap().to_owned();
    if !tags.iter().any(|tag| tag.term == sync_tag) {
        return Err(format!("The Sync Tag {} does not exist on master. Cannot sync.", &sync_tag).into());
    }

    let dashboards = config.service.instance_master
        .get_dashboards_by_tag(&sync_tag).await?;

    debug!("Master Dashboards with synctag: {:?}", &dashboards);

    for dashboard in &dashboards {
        config.service.replicate_to_slaves(dashboard).await?;
    }

    Ok(())
}
