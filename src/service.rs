use crate::api::dashboards::{Folder, FullDashboard};
use crate::config::Config;
use crate::dashboard_state::DashboardState;
use crate::error::GSError;
use crate::instance::GrafanaInstance;
use chrono::Local;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use log::{debug, error, info};
use tokio::time::Instant;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::instrument;

// base_url -> (folder title -> Folder)
pub type FolderMap = HashMap<String, HashMap<String, Folder>>;

/// Periodically synchronises all tagged dashboards across *all* instances.
#[derive(Debug, Clone)]
pub struct SyncService {
    cfg: Arc<Config>,
}

impl SyncService {

    /* Constructors */

    #[instrument(skip_all)]
    pub fn new(cfg: Config) -> Self {
        Self { cfg: Arc::new(cfg) }
    }

    /* Public API */

    /// Runs forever, every sync_cycle_interval
    #[instrument(skip_all)]
    pub async fn run(&self) -> Result<(), GSError> {
        let mut tick = tokio::time::interval(Duration::from_secs(self.cfg.sync_rate_mins * 60));
        let mut cycle = 0usize;

        loop {
            tick.tick().await;
            
            info!("=== sync-cycle #{cycle} ({}) ===", Local::now());

            let start = Instant::now();
            if let Err(e) = self.run_single_cycle(cycle).await {
                error!("cycle #{cycle} failed: {e}");
            }
            info!("=== finished sync-cycle #{cycle} in {:?}", start.elapsed());
            cycle += 1;
        }
    }

    /* Core Logic */

    async fn run_single_cycle(&self, cycle: usize) -> Result<(), GSError> {
        let mut state = DashboardState::new(self.cfg.instances.len());
        self.collect_dashboards(&mut state).await?;

        state.print_data_stats();

        let folder_map = self.
            mirror_folders(state.unique_folders().iter().map(|&c| c.to_owned()).collect())
            .await;

        let dashboards = Arc::new(
            state
                .diff(cycle != 0, self.cfg.sync_rate_mins)
                .into_iter()
                .map(|(uid, d)| (uid.to_owned(), RwLock::new(d)))
                .collect::<Vec<_>>(),
        );

        self.replicate_dashboards(dashboards, &folder_map).await?;

        self.purge_empty_folders().await
    }

    async fn collect_dashboards(
        &self,
        state: &mut DashboardState,
    ) -> Result<(), GSError> {
        let mut tasks = FuturesUnordered::new();

        for instance in &self.cfg.instances {
            let instance = instance.clone();
            let tag = self.cfg.sync_tag.clone();
            tasks.push(async move { fetch_full_dashboards(instance, &tag).await });
        }

        while let Some(res) = tasks.next().await {
            let (base_url, dashboards) = res?;
            state.add_set(base_url, dashboards);
        }

        Ok(())
    }

    async fn mirror_folders(&self, folders: HashSet<String>) -> FolderMap {
        let folders = Arc::new(folders);
        let mut tasks = FuturesUnordered::new();

        for instance in &self.cfg.instances {
            let instance = instance.clone();
            let folders = folders.clone();
            tasks.push(async move { ensure_folders_on_instance(instance, &folders).await });
        }

        let mut map = FolderMap::new();
        while let Some((url, folders)) = tasks.next().await {
            map.insert(url, folders);
        }
        map
    }

    async fn replicate_dashboards(
        &self,
        dashboards: Arc<Vec<(String, RwLock<Option<FullDashboard>>)>>,
        folder_map: &FolderMap,
    ) -> Result<(), GSError> {
        let folder_map = Arc::new(folder_map.clone());
        let mut tasks = FuturesUnordered::new();

        for instance in &self.cfg.instances {
            let instance = instance.clone();
            let dbs = dashboards.clone();
            let folders = folder_map.clone();
            tasks.push(tokio::spawn(async move {
                replicate_dashboards_on_instance(folders, dbs, instance).await
            }));
        }

        while let Some(res) = tasks.next().await {
            res??;
        }
        Ok(())
    }

    async fn purge_empty_folders(&self) -> Result<(), GSError> {
        for instance in &self.cfg.instances {
            instance.remove_empty_folders().await?;
        }
        Ok(())
    }
}

async fn fetch_full_dashboards(
    instance: GrafanaInstance,
    tag: &str
) -> Result<(String, Vec<FullDashboard>), GSError> {
    let mut dashboards = Vec::new();
    for d in instance.get_dashboards_by_tag(tag).await? {
        dashboards.push(instance.get_dashboard_full(&d.uid).await?);
    }
    Ok((instance.base_url().to_owned(), dashboards))
}

async fn ensure_folders_on_instance(
    instance: GrafanaInstance,
    folders: &HashSet<String>,
) -> (String, HashMap<String, Folder>) {
    let mut map = HashMap::new();
    for name in folders {
        if name == "General" {
            continue;
        }
        match instance.ensure_folder(name).await {
            Ok(folder) => {
                map.insert(folder.title.clone(), folder);
            }
            Err(e) => error!("{}: could not create folder '{name}': {e}", instance.base_url()),
        }
    }
    (instance.base_url().to_owned(), map)
}

async fn replicate_dashboards_on_instance(
    folder_map: Arc<FolderMap>,
    dashboards: Arc<Vec<(String, RwLock<Option<FullDashboard>>)>>,
    inst: GrafanaInstance,
) -> Result<(), GSError> {
    let folders = match folder_map.get(inst.base_url()) {
        Some(f) => f,
        None => {
            error!("{}: folder map missing (unauthorised?)", inst.base_url());
            return Ok(());
        }
    };

    let mut jobs = FuturesUnordered::new();

    for (uid, guard) in dashboards.iter() {
        let inst = inst.clone();
        let folders = folders.clone();
        jobs.push(async move {
            let maybe_dashboard = guard.read().await;
            match &*maybe_dashboard {
                Some(d) => {
                    let title = d.meta.folder_title
                        .as_deref()
                        .unwrap_or("");
                    let folder = folders.get(title);
                    inst.import_dashboard(d, folder, true).await?;
                }
                None => {
                    debug!("{}: deleting dashboard '{uid}'", inst.base_url());
                    // TODO: Only truly delete once I deem this stable
                    // inst.delete_dashboard(uid).await?;
                }
            }
            Ok::<_, GSError>(())
        });
    }

    while let Some(res) = jobs.next().await {
        res?; // bubble up any API error
    }
    Ok(())
}
