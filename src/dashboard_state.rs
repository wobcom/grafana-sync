use crate::api::dashboards::FullDashboard;
use chrono::Local;
use std::collections::{HashMap, HashSet};

pub struct DashboardState {
    dashboard_sets: HashMap<String, Vec<FullDashboard>>,
}

impl DashboardState {
    pub fn new() -> DashboardState {
        DashboardState {
            dashboard_sets: HashMap::new(),
        }
    }

    pub fn add_set(&mut self, base_url: String, dashboards: Vec<FullDashboard>) {
        self.dashboard_sets.insert(base_url, dashboards);
    }

    pub fn get_unique_folders(&self) -> HashSet<String> {
        self.dashboard_sets
            .values()
            .flatten()
            .map(|d| &d.meta.folder_title)
            .collect::<HashSet<_>>() // first collect references as to not make a hashset and throw away cloned values
            .into_iter()
            .cloned()
            .collect()
    }

    fn get_unequal_dashboard_uids(&self) -> HashSet<String> {
        let mut unequal_uids = HashSet::new();
        for (base_url1, dashboards1) in &self.dashboard_sets {
            for dashboard in dashboards1 {
                let mut is_synced = true;
                for (base_url2, dashboards2) in &self.dashboard_sets {
                    if base_url1 == base_url2 {
                        continue;
                    }

                    let other_dashboard = dashboards2
                        .iter()
                        .find(|&d| dashboard.dashboard.uid == d.dashboard.uid);

                    match other_dashboard {
                        Some(other_dashboard) => {
                            is_synced &= other_dashboard.dashboard.panels == dashboard.dashboard.panels &&
                                other_dashboard.dashboard.title == dashboard.dashboard.title
                        }
                        None => is_synced = false,
                    };

                    if !is_synced {
                        break;
                    }
                }
                if !is_synced {
                    unequal_uids.insert(dashboard.dashboard.uid.clone());
                }
            }
        }

        unequal_uids
    }

    fn get_unsynced_dashboards(&self) -> HashMap<String, Vec<Option<FullDashboard>>> {
        let unequal_uids = self.get_unequal_dashboard_uids();

        let mut unsynced_dashboards: HashMap<String, Vec<Option<FullDashboard>>> = HashMap::new();
        for dashboards in self.dashboard_sets.values() {
            for uid in &unequal_uids {
                let dashboard = dashboards.iter().find(|d| &d.dashboard.uid == uid);
                unsynced_dashboards
                    .entry(uid.clone()) // uhh... just stabilize raw_entry...
                    .or_default()
                    .push(dashboard.cloned());
            }
        }

        unsynced_dashboards
    }

    fn merge_dashboards(
        dashboards: &[Option<FullDashboard>],
        destructive: bool,
        sync_interval_mins: u64,
    ) -> Option<(String, Option<FullDashboard>)> {
        let Some(first) = dashboards.iter().filter_map(|d| d.as_ref()).next() else {
            return None;
        };

        let uid = first.dashboard.uid.clone();

        Some((uid, dashboards
            .iter()
            .fold(None, |merged, d| match (merged, d.clone()) {
                (None, d) => Some(d), // init
                (Some(None), None) => Some(None),
                (Some(Some(merged)), Some(d)) => {
                    Some(Some(if d.meta.updated > merged.meta.updated {
                        d
                    } else {
                        merged
                    }))
                }
                (Some(None), Some(d)) | (Some(Some(d)), None) => {
                    if destructive
                        && (Local::now() - d.meta.updated).num_minutes() as u64> sync_interval_mins
                    {
                        Some(None)
                    } else {
                        Some(Some(d))
                    }
                }
            }).flatten()))
    }

    fn merge_unsynced_dashboards(&self, destructive: bool, sync_interval_mins: u64) -> Vec<(String, Option<FullDashboard>)> {
        let unsynced_set_sets = self.get_unsynced_dashboards();

        unsynced_set_sets
            .values()
            .filter_map(|d| Self::merge_dashboards(&d[..], destructive, sync_interval_mins))
            .collect()
    }

    pub fn get_new_dashboards(&self, destructive: bool, sync_interval_mins: u64) -> Vec<(String, Option<FullDashboard>)> {
        self.merge_unsynced_dashboards(destructive, sync_interval_mins)
    }
}
