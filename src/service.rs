use std::time::Duration;
use chrono::Local;
use futures::future;
use log::{debug, error, info};
use tokio::time::sleep;
use tracing::instrument;
use crate::api::dashboards::{Folder, SimpleDashboard};
use crate::config::Config;
use crate::error::GSError;
use crate::instance::GrafanaInstance;

#[derive(Debug, Clone)]
pub struct SyncService {
    config: Config
}

impl SyncService {
    #[instrument]
    pub fn new(config: Config) -> Self {
        Self {
            config
        }
    }

    pub async fn wait_for_next_sync(&self) {
        info!("Commencing next sync at {}", (Local::now() + Duration::from_mins(self.config.service.sync_rate_mins)).to_rfc2822());
        sleep(Duration::from_mins(self.config.service.sync_rate_mins)).await;
    }

    pub async fn replicate_to_slaves(&mut self, dashboard: &SimpleDashboard) -> Result<(), GSError> {
        info!("Starting replication of dashboard \"{}/{}\"", dashboard.folder_title, dashboard.title);
        let mut slave_folders: Vec<(&mut GrafanaInstance, Folder)> = Vec::new();
        for slave in &mut self.config.service.instance_slaves {
            match slave.ensure_folder(&dashboard.folder_title).await {
                Ok(folder) => {
                    slave_folders.push((slave, folder));
                }
                Err(e) => {
                    error!("Couldn't sync folder for instance {}. Error: {e}", slave.base_url());
                }
            }
        }

        let mut full_dashboard = self.config.service.instance_master.get_dashboard_full(&dashboard.uid).await?;
        for (slave, folder) in slave_folders {
            // Override or create new dashboard
            let old_dashboard = slave.get_first_dashboard_in_folder_by_name(&folder.uid, &dashboard.title).await?;
            match &old_dashboard {
                Some(d) => info!("Performing overwriting dashboard sync. Target: {}", d.uid),
                None => info!("Performing new dashboard sync"),
            }
            full_dashboard.sanitize(old_dashboard.as_ref().map(|d| d.uid.as_str()));

            slave.import_dashboard(&full_dashboard, &folder, true).await?;
        }

        Ok(())
    }

    #[instrument]
    pub async fn run(&mut self) -> Result<(), GSError>{
        loop {
            let tags = self.config.service.instance_master.get_tags().await?;
            debug!("Master Tags: {tags:?}");

            let sync_tag = self.config.service.instance_master.sync_tag().unwrap().to_owned();
            if !tags.iter().any(|tag| tag.term == sync_tag) {
                error!("The sync tag {} does not exist on the master. Cannot sync.", sync_tag);
                self.wait_for_next_sync().await;
                continue;
            }

            let dashboards = self.config.service.instance_master
                .get_dashboards_by_tag(&sync_tag).await?;

            debug!("Master Dashboards with synctag: {:?}", &dashboards);

            let tasks: Vec<_> = dashboards
                .into_iter()
                .map(|dashboard| {
                    let mut service = self.clone();
                    tokio::spawn(async move {
                        service.replicate_to_slaves(&dashboard).await
                    })
            }).collect();

            for a in future::join_all(tasks).await {
                match a {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => error!("Couldn't sync dashboard. Error: {e}"),
                    Err(e) => error!("Couldn't sync dashboard. Error: {e}")
                }
            }

            self.wait_for_next_sync().await;
        }
    }
}