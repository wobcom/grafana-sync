use crate::error::GSError;
use crate::instance::GrafanaInstance;
use chrono::{DateTime, Local};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::instrument;

#[derive(Debug, Clone, Deserialize)]
pub struct Tag {
    pub term: String,
    pub count: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleDashboard {
    pub id: u32,
    pub uid: String,
    pub title: String,
    pub uri: String,
    pub url: String,
    pub slug: String,
    #[serde(alias = "type")]
    pub type_name: String,
    pub tags: Vec<String>,
    pub is_starred: bool,
    pub folder_id: Option<u32>,
    pub folder_uid: Option<String>,
    pub folder_title: Option<String>,
    pub folder_url: Option<String>,
    pub sort_meta: u32,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullDashboardMeta {
    pub annotations_permissions: AnnotationsPermissions,
    pub can_admin: bool,
    pub can_delete: bool,
    pub can_edit: bool,
    pub can_save: bool,
    pub can_star: bool,
    pub created: DateTime<Local>,
    pub created_by: String,
    pub expires: DateTime<Local>,
    pub folder_id: Option<i64>,
    pub folder_title: Option<String>,
    pub folder_uid: Option<String>,
    pub folder_url: Option<String>,
    pub has_acl: bool,
    pub is_folder: bool,
    pub provisioned: bool,
    pub provisioned_external_id: String,
    pub slug: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub updated: DateTime<Local>,
    pub updated_by: String,
    pub url: String,
    pub version: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullDashboardData {
    pub annotations: serde_json::Value,
    pub editable: bool,
    pub fiscal_year_start_month: i32,
    pub graph_tooltip: i32,
    pub links: Vec<String>,
    pub panels: Vec<serde_json::Value>,
    pub schema_version: i32,
    pub tags: Vec<String>,
    pub templating: serde_json::Value,
    pub time: Option<serde_json::Value>,
    pub timepicker: serde_json::Value,
    pub timezone: String,
    pub title: String,
    pub uid: String,
    pub version: i32,
    pub week_start: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationsPermissions {
    pub dashboard: AnnotationsDashboardMeta,
    pub organization: AnnotationsOrganizationMeta,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationsDashboardMeta {
    pub can_add: bool,
    pub can_delete: bool,
    pub can_edit: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationsOrganizationMeta {
    pub can_add: bool,
    pub can_delete: bool,
    pub can_edit: bool,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Folder {
    pub id: u32,
    pub uid: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullDashboard {
    pub dashboard: FullDashboardData,
    pub meta: FullDashboardMeta,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardImportBody {
    pub dashboard: FullDashboardData,
    pub folder_uid: Option<String>,
    pub inputs: Vec<serde_json::Value>,
    pub overwrite: bool,
    // pub path: String,
    // pub plugin_id: String,
}

#[allow(dead_code)]
impl GrafanaInstance {
    pub async fn get_tags(&self) -> Result<Vec<Tag>, GSError> {
        let endpoint = format!("{}/api/dashboards/tags", &self.base_url());
        let client = self.client();

        let response = client.get(endpoint).send().await?.error_for_status()?;
        let text = response.text().await?;

        Ok(serde_json::from_str::<Vec<Tag>>(&text)?)
    }

    pub async fn get_dashboards_by_tag(&self, tag: &str) -> Result<Vec<SimpleDashboard>, GSError> {
        let endpoint = format!("{}/api/search", &self.base_url());
        let client = self.client();

        let response = client
            .get(endpoint)
            .query(&[("tag", tag), ("permission", "View"), ("sort", "alpha-asc")])
            .send()
            .await?
            .error_for_status()?;
        let text = response.text().await?;

        Ok(serde_json::from_str::<Vec<SimpleDashboard>>(&text)?)
    }

    #[allow(dead_code)]
    pub async fn get_dashboards_in_folder(
        &self,
        folder_uid: &str,
    ) -> Result<Vec<SimpleDashboard>, GSError> {
        let endpoint = format!("{}/api/search", &self.base_url());
        let client = self.client();

        let response = client
            .get(endpoint)
            .query(&[
                ("folderUIDs", folder_uid),
                ("permission", "View"),
                ("sort", "alpha-asc"),
            ])
            .send()
            .await?
            .error_for_status()?;
        let text = response.text().await?;

        Ok(serde_json::from_str::<Vec<SimpleDashboard>>(&text)?)
    }

    pub async fn get_dashboard_full(&self, uid: &str) -> Result<FullDashboard, GSError> {
        let endpoint = format!("{}/api/dashboards/uid/{}", &self.base_url(), uid,);
        let client = self.client();

        debug!("Requesting full dashboard of uid: {}", uid);

        let response = client.get(endpoint).send().await?.error_for_status()?;
        let text = response.text().await?;

        Ok(serde_json::from_str(&text)?)
    }

    #[allow(dead_code)]
    pub async fn delete_dashboard(&self, uid: &str) -> Result<(), GSError> {
        let endpoint = format!("{}/api/dashboards/uid/{}", &self.base_url(), uid,);
        let client = self.client();

        debug!("Deleting dashboard with uid: {}", uid);

        client.delete(endpoint).send().await?.error_for_status()?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete_dashboards_in_folder_by_name(
        &self,
        folder_uid: &str,
        dashboard_name: &str,
    ) -> Result<(), GSError> {
        let mut dashboards = self.get_dashboards_in_folder(folder_uid).await?;

        dashboards.retain(|d| d.title == dashboard_name);

        info!(
            "Deleting {} to-be-synced dashboards from the slave",
            dashboards.len()
        );

        for dashboard in dashboards {
            self.delete_dashboard(&dashboard.uid).await?;
        }

        Ok(())
    }

    #[instrument]
    pub async fn import_dashboard(
        &self,
        dashboard: &FullDashboard,
        folder: Option<&Folder>,
        overwrite: bool,
    ) -> Result<(), GSError> {
        let base_url = self.base_url().to_string();
        let endpoint = format!("{}/api/dashboards/import", base_url);
        let folder_uid = folder
            .map(|f| f.uid.clone());

        info!(
            "Starting replication of dashboard \"{}\" onto {}",
            dashboard.meta.url, base_url
        );

        let body = DashboardImportBody {
            dashboard: dashboard.dashboard.clone(),
            folder_uid,
            inputs: vec![],
            overwrite,
            // path: "".to_string(),
            // plugin_id: "".to_string(),
        };

        let client = self.client();
        let response = client.post(endpoint).json(&body).send().await?;
        let status = response.status();

        if status.as_u16() == 412 {
            warn!("Dashboard already exists, but overwriting was turned off. Skipping...");
            return Ok(());
        }

        let response = response.error_for_status()?;

        info!(
            "Replication of dashboard {} to {} successful",
            dashboard.meta.url, base_url
        );

        let text = response.text().await?;
        debug!("Import response: {}", text);

        Ok(())
    }

    pub async fn get_dashboard_full_bulk(
        &self,
        dashboards: &[SimpleDashboard],
    ) -> Result<Vec<(SimpleDashboard, RwLock<FullDashboard>)>, GSError> {
        let mut full_dashboards = Vec::new();
        for dashboard in dashboards {
            info!(
                "Prefetching full dashboard: {}/{}",
                dashboard.folder_title
                    .as_deref()
                    .unwrap_or(""), 
                dashboard.title
            );
            let full_dashboard = self.get_dashboard_full(&dashboard.uid).await?;

            full_dashboards.push((dashboard.clone(), RwLock::new(full_dashboard)))
        }
        Ok(full_dashboards)
    }
}
