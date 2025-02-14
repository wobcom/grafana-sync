use crate::api::dashboards::{Folder, FullDashboard};
use crate::config::Config;
use crate::dashboard_state::DashboardState;
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

// HashMap<Instance Base URL, HashMap<Folder Name, Folder>>
pub type FolderMap = HashMap<String, HashMap<String, Folder>>;

impl SyncService {
    #[instrument]
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn wait_for_next_sync(&self) {
        info!(
            "Commencing next sync at {}",
            (Local::now() + Duration::from_mins(self.config.sync_rate_mins)).to_rfc2822()
        );
        sleep(Duration::from_mins(self.config.sync_rate_mins)).await;
    }

    pub async fn replicate_folders_to_instances(&self, folders: &HashSet<String>) -> FolderMap {
        let mut created_folders: HashMap<String, HashMap<String, Folder>> = HashMap::new();

        for instance in &self.config.instances {
            for folder in folders.iter() {
                match instance.ensure_folder(folder).await {
                    Ok(folder) => {
                        created_folders
                            .entry(instance.base_url().to_string())
                            .or_default()
                            .insert(folder.title.clone(), folder);
                    }
                    Err(e) => {
                        error!(
                            "Couldn't sync folder for instance {}. Error: {e}",
                            instance.base_url()
                        );
                    }
                }
            }
        }

        created_folders
    }

    pub async fn replicate_dashboards(
        &self,
        full_dashboards: Arc<Vec<(String, RwLock<Option<FullDashboard>>)>>,
        folder_map: &FolderMap,
    ) -> Result<(), GSError> {
        let folder_map = Arc::new(folder_map.clone());

        let tasks: Vec<JoinHandle<Result<(), GSError>>> = self
            .config
            .instances
            .iter()
            .map(|instance| {
                let folder_map = folder_map.clone();
                let full_dashboards = full_dashboards.clone();
                let instance = instance.clone();
                tokio::spawn(async move {
                    Self::replicate_dashboards_to_instance(folder_map, full_dashboards, instance).await
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

    async fn replicate_dashboards_to_instance(
        folder_map: Arc<HashMap<String, HashMap<String, Folder>>>,
        full_dashboards: Arc<Vec<(String, RwLock<Option<FullDashboard>>)>>,
        instance: GrafanaInstance,
    ) -> Result<(), GSError> {
        for full_dashboard in full_dashboards.iter() {
            Self::replicate_dashboard_to_instance(&folder_map, &instance, full_dashboard).await?;
        }
        Ok(())
    }

    async fn replicate_dashboard_to_instance(
        folder_map: &Arc<HashMap<String, HashMap<String, Folder>>>,
        instance: &GrafanaInstance,
        full_dashboard: &(String, RwLock<Option<FullDashboard>>),
    ) -> Result<(), GSError> {
        let dashboard = full_dashboard.1.read().await;

        let Some(dashboard) = dashboard.as_ref() else {
            info!("Deleting dashboard \"{}\" on instance {}", full_dashboard.0, instance.base_url());
            // instance.delete_dashboard(&full_dashboard.0).await?;
            return Ok(());
        };

        info!(
            "Replicating dashboard \"{}/{}\" to {}",
            dashboard.meta.folder_title,
            dashboard.dashboard.title,
            instance.base_url()
        );

        let Some(instance_folder_map) = folder_map.get(instance.base_url()) else {
            error!("Instance {} is out of sync (Unauthorized?)", instance.base_url());
            return Ok(());
        };

        let Some(folder) = instance_folder_map.get(&dashboard.meta.folder_title) else {
            error!("Instance {} is out of sync. (Unauthorized?)", instance.base_url());
            return Ok(());
        };

        instance
            .import_dashboard(&dashboard, folder, true)
            .await?;

        Ok(())
    }

    pub async fn remove_empty_folders_on_all_instances(&self) -> Result<(), GSError> {
        for instance in &self.config.instances {
            instance.remove_empty_folders().await?;
        }

        Ok(())
    }

    pub async fn collect_all_sync_dashboards(
        &self,
        dashboard_state: &mut DashboardState,
    ) -> Result<(), GSError> {
        for instance in &self.config.instances {
            let dashboards = instance.get_dashboards_by_tag(&self.config.sync_tag).await?;
            let mut full_dashboards = Vec::new();

            for dashboard in dashboards {
                let full_dashboard = instance.get_dashboard_full(&dashboard.uid).await?;
                full_dashboards.push(full_dashboard);
            }

            dashboard_state.add_set(instance.base_url().to_string(), full_dashboards);
        }

        Ok(())
    }

    pub async fn run_sync_cycle(&self, cycle_count: usize) -> Result<(), GSError> {
        info!("Fetching all full dashboards with sync tag on all instances");

        let mut dashboard_state = DashboardState::new();
        self.collect_all_sync_dashboards(&mut dashboard_state)
            .await?;

        let folder_map = self
            .replicate_folders_to_instances(&dashboard_state.get_unique_folders())
            .await;

        let new_dashboards = Arc::new(
            dashboard_state
                .get_new_dashboards(cycle_count == 0, self.config.sync_rate_mins)
                .into_iter()
                .map(|d| (d.0, RwLock::new(d.1)))
                .collect::<Vec<_>>(),
        );

        self.replicate_dashboards(new_dashboards, &folder_map)
            .await?;

        self.remove_empty_folders_on_all_instances().await?;

        Ok(())
    }

    #[instrument]
    pub async fn run(&mut self) -> Result<(), GSError> {
        let mut cycle_count = 0;
        loop {
            match self.run_sync_cycle(cycle_count).await {
                Ok(_) => info!("Sync completed successfully"),
                Err(e) => error!("Sync error: {e}"),
            }
            self.wait_for_next_sync().await;
            cycle_count += 1;
        }
    }
}
