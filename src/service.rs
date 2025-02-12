use crate::api::dashboards::{Folder, SimpleDashboard};
use crate::config::Config;
use crate::error::GSError;
use chrono::Local;
use futures::future;
use log::{debug, error, info};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Instant};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct SyncService {
    config: Config,
}

// HashMap<Slave Base URL, HashMap<Folder Name, Folder>>
pub type FolderMap = HashMap<String, HashMap<String, Folder>>;

impl SyncService {
    #[instrument]
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn wait_for_next_sync(&self) {
        info!(
            "Commencing next sync at {}",
            (Local::now() + Duration::from_mins(self.config.service.sync_rate_mins)).to_rfc2822()
        );
        sleep(Duration::from_mins(self.config.service.sync_rate_mins)).await;
    }

    pub async fn replicate_folders_to_slaves(&self, dashboards: &[SimpleDashboard]) -> FolderMap {
        let unique_folders = dashboards
            .iter()
            .map(|d| &d.folder_title)
            .collect::<HashSet<_>>();

        let mut created_folders: HashMap<String, HashMap<String, Folder>> = HashMap::new();

        for slave in &self.config.service.instance_slaves {
            for folder in unique_folders.iter() {
                match slave.ensure_folder(folder).await {
                    Ok(folder) => {
                        created_folders
                            .entry(slave.base_url().to_string())
                            .or_default()
                            .insert(folder.title.clone(), folder);
                    }
                    Err(e) => {
                        error!(
                            "Couldn't sync folder for instance {}. Error: {e}",
                            slave.base_url()
                        );
                    }
                }
            }
        }

        created_folders
    }

    pub async fn replicate_dashboards_to_slaves(
        &self,
        dashboards: &[SimpleDashboard],
        folder_map: &FolderMap,
    ) -> Result<(), GSError> {
        let mut full_dashboards = Vec::new();
        for dashboard in dashboards {
            let full_dashboard = self
                .config
                .service
                .instance_master
                .get_dashboard_full(&dashboard.uid)
                .await?;

            full_dashboards.push((dashboard.clone(), full_dashboard))
        }

        let instance_slaves = Arc::new(self.config.service.instance_slaves.clone());
        let folder_map = Arc::new(folder_map.clone());

        let tasks: Vec<JoinHandle<Result<(), GSError>>> = full_dashboards
            .to_vec()
            .into_iter()
            .map(|(dashboard, mut full_dashboard)| {
                let slaves = instance_slaves.clone();
                let folder_map = folder_map.clone();
                tokio::task::spawn(async move {
                    info!(
                        "Starting replication of dashboard \"{}/{}\"",
                        dashboard.folder_title, dashboard.title
                    );

                    for slave in slaves.iter() {
                        let Some(slave_folder_map) = folder_map.get(slave.base_url()) else {
                            error!("Slave {} is out of sync (Unauthorized?)", slave.base_url());
                            continue;
                        };

                        let Some(folder) = slave_folder_map.get(&dashboard.folder_title) else {
                            error!("Slave {} is out of sync. (Unauthorized?)", slave.base_url());
                            continue;
                        };

                        // Override or create new dashboard
                        let old_dashboard = slave
                            .get_first_dashboard_in_folder_by_name(&folder.uid, &dashboard.title)
                            .await?;
                        match &old_dashboard {
                            Some(d) => {
                                info!("Performing overwriting dashboard sync. Target: {}", d.uid)
                            }
                            None => info!("Performing new dashboard sync"),
                        }
                        full_dashboard.sanitize(old_dashboard.as_ref().map(|d| d.uid.as_str()));

                        slave
                            .import_dashboard(&full_dashboard, &folder, true)
                            .await?;
                    }

                    Ok(())
                })
            })
            .collect();


        let start = Instant::now();
        future::join_all(tasks).await;
        println!("All dashboards were synced in {:?}", start.elapsed());

        Ok(())
    }

    #[instrument]
    pub async fn run(&mut self) -> Result<(), GSError> {
        loop {
            let tags = self.config.service.instance_master.get_tags().await?;
            debug!("Master Tags: {tags:?}");

            let sync_tag = self
                .config
                .service
                .instance_master
                .sync_tag()
                .unwrap()
                .to_owned();
            if !tags.iter().any(|tag| tag.term == sync_tag) {
                error!(
                    "The sync tag {} does not exist on the master. Cannot sync.",
                    sync_tag
                );
                self.wait_for_next_sync().await;
                continue;
            }

            let dashboards = self
                .config
                .service
                .instance_master
                .get_dashboards_by_tag(&sync_tag)
                .await?;

            debug!("Master Dashboards with synctag: {:?}", &dashboards);

            let folder_map = self.replicate_folders_to_slaves(&dashboards).await;
            self.replicate_dashboards_to_slaves(&dashboards, &folder_map)
                .await?;

            self.wait_for_next_sync().await;
        }
    }
}
