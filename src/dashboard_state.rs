use crate::api::dashboards::FullDashboard;
use chrono::{DateTime, Local};
use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};

type Dashboards<'a> = HashMap<&'a str, Vec<&'a FullDashboard>>;
type SetName        = String;

#[derive(Debug, Clone)]
pub struct DashboardState {
    sets: HashMap<SetName, Vec<FullDashboard>>,
    instance_count: usize,
}

impl DashboardState {

    /* Constructors */

    pub fn new(instance_count: usize) -> DashboardState {
        Self { 
            sets: HashMap::new(),
            instance_count
        }
    }

    pub fn add_set(&mut self, base_url: String, dashboards: Vec<FullDashboard>) {
        self.sets.insert(base_url, dashboards);
    }

    /* public API */

    pub fn diff(
        &self,
        destructive: bool,
        sync_interval_mins: u64,
    ) -> Vec<(&str, Option<FullDashboard>)> {
        let by_uid = index_by_uid(&self.sets);

        by_uid
            .into_iter()
            .filter_map(|(uid, dashboards)| {
                merge_dashboards(uid, &dashboards, destructive, sync_interval_mins, self.instance_count)
            })
            .collect()
    }

    pub fn unique_folders(&self) -> HashSet<&str> {
        self.sets
            .values()
            .flat_map(|v| v.iter().map(|d| d.meta.folder_title.as_ref()))
            .filter_map(|d| d.map(|s| s.as_str()))
            .collect()
    }

    pub fn print_data_stats(&self) {
        for (name, dbs) in &self.sets {
            info!("{name}: {} sync dashboards", dbs.len());
        }
    }
}

fn index_by_uid(sets: &HashMap<SetName, Vec<FullDashboard>>) -> Dashboards<'_> {
    let mut map: Dashboards = HashMap::new();
    for dashboards in sets.values() {
        for d in dashboards {
            map.entry(d.dashboard.uid.as_str()).or_default().push(d);
        }
    }
    map
}

/// Decide whether the dashboards with the same UID are **all** identical.
/// If not, determine which concrete dashboard should “win”.
fn merge_dashboards<'a>(
    uid: &'a str,
    dashboards: &[&FullDashboard],
    destructive: bool,
    sync_interval_mins: u64,
    instance_count: usize,
) -> Option<(&'a str, Option<FullDashboard>)> {
    debug!("{uid}: {:?}", dashboards.iter().map(|d| &d.dashboard.title).collect::<Vec<_>>());
    let first = dashboards.first()?;

    // Fast track: If all dashboards are synced already
    if dashboards.len() == instance_count 
        && dashboards
        .iter()
        .all(|d| dashboards_equal(first, d))
    {
        return None;
    }

    // Otherwise pick the newest
    let newest = dashboards
        .iter()
        .copied()
        .max_by_key(|d| d.meta.updated)?;

    // Delete if matching criteria to determine it was deleted
    let now: DateTime<Local> = Local::now();
    let age_mins: u64        = (now - newest.meta.updated).num_minutes() as u64;
    let delete_outdated      = destructive && age_mins > sync_interval_mins * 2;

    let result = if delete_outdated {
        warn!("Dashboard {} will be deleted", newest.dashboard.title);
        None
    } else {
        Some(newest.clone())
    };

    Some((uid, result))
}

#[inline]
fn dashboards_equal(a: &FullDashboard, b: &FullDashboard) -> bool {
    a.dashboard.uid == b.dashboard.uid
        && a.dashboard.title == b.dashboard.title
        && a.dashboard.tags == b.dashboard.tags
        && a.dashboard.panels == b.dashboard.panels
        && a.dashboard.graph_tooltip == b.dashboard.graph_tooltip
}
