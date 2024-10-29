use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::instrument;
use crate::error::GSError;
use crate::instance::GrafanaInstance;

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
    pub folder_id: u32,
    pub folder_uid: String,
    pub folder_title: String,
    pub folder_url: String,
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
    pub created: String,
    pub created_by: String,
    pub expires: String,
    pub folder_id: i64,
    pub folder_title: String,
    pub folder_uid: String,
    pub folder_url: String,
    pub has_acl: bool,
    pub is_folder: bool,
    pub provisioned: bool,
    pub provisioned_external_id: String,
    pub slug: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub updated: String,
    pub updated_by: String,
    pub url: String,
    pub version: i64,
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

#[derive(Debug, Clone, Deserialize)]
pub struct FullDashboard {
    pub dashboard: serde_json::Value,
    pub meta: FullDashboardMeta,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardImportBody {
    pub dashboard: serde_json::Value,
    pub folder_uid: String,
    pub inputs: Vec<serde_json::Value>,
    pub overwrite: bool,
    // pub path: String,
    // pub plugin_id: String,
}

impl GrafanaInstance {
    pub async fn get_tags(&mut self) -> Result<Vec<Tag>, GSError> {
        let endpoint = format!("{}{}", &self.base_url(), "/api/dashboards/tags");
        let client = self.client()?;

        let response = client.get(endpoint)
            .send()
            .await?
            .error_for_status()?;
        let text = response.text().await?;

        Ok(serde_json::from_str::<Vec<Tag>>(&text)?)
    }

    pub async fn get_dashboards_by_tag(&mut self, tag: &str) -> Result<Vec<SimpleDashboard>, GSError> {
        let endpoint = format!("{}{}", &self.base_url(), "/api/search");
        let client = self.client()?;

        let response = client
            .get(endpoint)
            .query(&[("tag", tag), ("permission", "View"), ("sort", "alpha-asc")])
            .send()
            .await?
            .error_for_status()?;
        let text = response.text().await?;

        Ok(serde_json::from_str::<Vec<SimpleDashboard>>(&text)?)
    }

    pub async fn get_dashboards_in_folder(&mut self, folder_uid: &str) -> Result<Vec<SimpleDashboard>, GSError> {
        let endpoint = format!("{}{}", &self.base_url(), "/api/search");
        let client = self.client()?;

        let response = client
            .get(endpoint)
            .query(&[("folderUIDs", folder_uid), ("permission", "View"), ("sort", "alpha-asc")])
            .send()
            .await?
            .error_for_status()?;
        let text = response.text().await?;

        Ok(serde_json::from_str::<Vec<SimpleDashboard>>(&text)?)
    }

    pub async fn get_dashboard_full(&mut self, uid: &str) -> Result<FullDashboard, GSError> {
        let endpoint = format!("{}{}", &self.base_url(), format!("/api/dashboards/uid/{}", uid));
        let client = self.client()?;

        debug!("Requesting full dashboard of uid: {}", uid);

        let response = client
            .get(endpoint)
            .send()
            .await?
            .error_for_status()?;
        let text = response.text().await?;

        Ok(serde_json::from_str(&text)?)
    }

    pub async fn delete_dashboard(&mut self, uid: &str) -> Result<(), GSError> {
        let endpoint = format!("{}{}", &self.base_url(), format!("/api/dashboards/uid/{}", uid));
        let client = self.client()?;

        debug!("Deleting dashboard with uid: {}", uid);

        client
            .delete(endpoint)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    #[instrument]
    pub async fn import_dashboard(&mut self, dashboard: &FullDashboard, folder: &Folder, overwrite: bool) -> Result<(), GSError> {
        let base_url = self.base_url().to_string();
        let endpoint = format!("{}{}", base_url, "/api/dashboards/import");

        info!("Syncing dashboard \"{}\" to {}", dashboard.meta.url, base_url);

        let body = DashboardImportBody {
            dashboard: dashboard.dashboard.clone(),
            folder_uid: folder.uid.clone(),
            inputs: vec![],
            overwrite,
            // path: "".to_string(),
            // plugin_id: "".to_string(),
        };

        let client = self.client()?;
        let response = client
            .post(endpoint)
            .json(&body)
            .send()
            .await?;
        let status = response.status();

        if status.as_u16() == 412 {
            warn!("Dashboard already exists, but overwriting was turned off. Skipping...");
            return Ok(());
        }

        let response = response.error_for_status()?;

        info!("Replication of dashboard {} to {} successful", dashboard.meta.url, base_url);

        let text = response.text().await?;
        debug!("Import response: {}", text);

        Ok(())
    }
}

impl FullDashboard {
    pub fn sanitize(&mut self) {
        if let Value::Object(ref mut map) = self.dashboard {
            map.remove("uid");
            map.remove("id");
        }
    }
}