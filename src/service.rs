use crate::api::dashboards::{Folder, FullDashboard, SimpleDashboard};
use crate::config::Config;
use crate::error::GSError;
use crate::instance::GrafanaInstance;
use chrono::Local;
use futures::future;
use log::{debug, error, info};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
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
        full_dashboards: Arc<Vec<(SimpleDashboard, RwLock<FullDashboard>)>>,
        folder_map: &FolderMap,
    ) -> Result<(), GSError> {
        let folder_map = Arc::new(folder_map.clone());

        let tasks: Vec<JoinHandle<Result<(), GSError>>> = self
            .config
            .service
            .instance_slaves
            .iter()
            .map(|slave| {
                let folder_map = folder_map.clone();
                let full_dashboards = full_dashboards.clone();
                let slave = slave.clone();
                tokio::spawn(async move {
                    Self::replicate_dashboards_to_slave(folder_map, full_dashboards, slave).await
                })
            })
            .collect();

        debug!("Starting import task executor");
        let start = Instant::now();
        future::join_all(tasks)
            .await
            .into_iter()
            .filter_map(|r| r.err())
            .for_each(|r| error!("Error occurred while syncing dashboard: {r}"));
        info!("All dashboards were synced in {:?}", start.elapsed());

        Ok(())
    }

    async fn prefetch_full_dashboards_from_master(
        &self,
        dashboards: &[SimpleDashboard],
    ) -> Result<Vec<(SimpleDashboard, RwLock<FullDashboard>)>, GSError> {
        let mut full_dashboards = Vec::new();
        for dashboard in dashboards {
            info!(
                "Prefetching full dashboard: {}/{}",
                dashboard.folder_title, dashboard.title
            );
            let full_dashboard = self
                .config
                .service
                .instance_master
                .get_dashboard_full(&dashboard.uid)
                .await?;

            full_dashboards.push((dashboard.clone(), RwLock::new(full_dashboard)))
        }
        Ok(full_dashboards)
    }

    async fn replicate_dashboards_to_slave(
        folder_map: Arc<HashMap<String, HashMap<String, Folder>>>,
        full_dashboards: Arc<Vec<(SimpleDashboard, RwLock<FullDashboard>)>>,
        slave: GrafanaInstance,
    ) -> Result<(), GSError> {
        for (dashboard, full_dashboard) in full_dashboards.iter() {
            Self::replicate_dashboard_to_slave(&folder_map, &slave, dashboard, full_dashboard)
                .await?;
        }
        Ok(())
    }

    async fn replicate_dashboard_to_slave(
        folder_map: &Arc<HashMap<String, HashMap<String, Folder>>>,
        slave: &GrafanaInstance,
        dashboard: &SimpleDashboard,
        full_dashboard: &RwLock<FullDashboard>,
    ) -> Result<(), GSError> {
        info!(
            "Replicating dashboard \"{}/{}\" to {}",
            dashboard.folder_title, dashboard.title, slave.base_url()
        );

        let Some(slave_folder_map) = folder_map.get(slave.base_url()) else {
            error!("Slave {} is out of sync (Unauthorized?)", slave.base_url());
            return Ok(());
        };

        let Some(folder) = slave_folder_map.get(&dashboard.folder_title) else {
            error!("Slave {} is out of sync. (Unauthorized?)", slave.base_url());
            return Ok(());
        };

        let mut full_dashboard = full_dashboard.write().await;
        full_dashboard.sanitize(Some(&dashboard.uid));
        slave
            .import_dashboard(&full_dashboard, folder, true)
            .await?;

        Ok(())
    }

    pub async fn delete_all_synctag_dashboards_from_slaves(&self, sync_tag: &str) -> Result<(), GSError> {
        let tasks = self.config.service.instance_slaves.iter().map(|slave| {
            let slave = slave.clone();
            let sync_tag = sync_tag.to_string();
            tokio::spawn(async move {
                info!("Fetching all synced dashboards on slave {}", slave.base_url());
                let dashboards = slave.get_dashboards_by_tag(&sync_tag).await?;

                for dashboard in dashboards {
                    info!("{dashboard:?}");
                    info!("Deleting synced dashboard {}/{}", dashboard.folder_title, dashboard.title);
                    slave.delete_dashboard(&dashboard.uid).await?;
                }

                Ok(())
            })
        }).collect::<Vec<_>>();

        future::join_all(tasks)
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .next()
            .unwrap_or(Ok(()))
    }

    pub async fn delete_empty_folders_on_slaves(&self) -> Result<(), GSError> {
        for slave in &self.config.service.instance_slaves {
            let all_folders = slave.get_all_folders().await?;

            for folder in all_folders {
                if slave.get_dashboards_in_folder(&folder.uid).await?.is_empty() {
                    info!("Deleting empty folder {}", folder.title);
                    slave.remove_folder(&folder.uid).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn run_sync_cycle(&self) -> Result<(), GSError> {
        let sync_tag = self
            .config
            .service
            .instance_master
            .sync_tag()
            .expect("Sync tag should always be set for a master instance");

        self.check_sync_tag_exists_on_master(sync_tag).await?;

        info!("Fetching all dashboards with sync tag on {}", self.config.service.instance_master.base_url());
        let dashboards = self
            .config
            .service
            .instance_master
            .get_dashboards_by_tag(sync_tag)
            .await?;

        debug!("Master Dashboards with synctag: {:?}", &dashboards);

        info!("Prefetching all sync dashboards");
        let full_dashboards = self
            .prefetch_full_dashboards_from_master(&dashboards)
            .await?;

        self.delete_all_synctag_dashboards_from_slaves(sync_tag).await?;

        let folder_map = self.replicate_folders_to_slaves(&dashboards).await;

        let full_dashboards = Arc::new(full_dashboards);
        self.replicate_dashboards_to_slaves(full_dashboards, &folder_map)
            .await?;

        self.delete_empty_folders_on_slaves().await?;

        Ok(())
    }

    async fn check_sync_tag_exists_on_master(&self, sync_tag: &str) -> Result<(), GSError> {
        let tags = self.config.service.instance_master.get_tags().await?;
        debug!("Master Tags: {tags:?}");

        if !tags.iter().any(|tag| tag.term == sync_tag) {
            Err(GSError::SyncTagMissing(sync_tag.to_string()))
        } else {
            Ok(())
        }
    }

    #[instrument]
    pub async fn run(&mut self) -> Result<(), GSError> {
        loop {
            match self.run_sync_cycle().await {
                Ok(_) => info!("Sync completed successfully"),
                Err(e) => error!("Sync error: {e}"),
            }
            self.wait_for_next_sync().await;
        }
    }
}
