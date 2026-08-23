use herta_core::{HbError, HbResult};
use serde::{Deserialize, Serialize};

use crate::{DbClient, schema::database_error};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebProject {
    pub name: String,
    pub alias: Option<String>,
    pub spa_fallback: bool,
    pub cache_control: String,
    pub not_found: Option<String>,
    pub deployed_at: String,
}

pub struct WebProjectManager<'a> {
    db: &'a DbClient,
}

impl<'a> WebProjectManager<'a> {
    pub fn new(db: &'a DbClient) -> Self {
        Self { db }
    }

    pub async fn list(&self) -> HbResult<Vec<WebProject>> {
        let mut response = self
            .db
            .inner()
            .query("SELECT * OMIT id FROM _web_projects ORDER BY name ASC")
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let records: Vec<serde_json::Value> = response.take(0).map_err(database_error)?;
        records
            .into_iter()
            .map(|value| serde_json::from_value(value).map_err(database_error))
            .collect()
    }

    pub async fn get(&self, name: &str) -> HbResult<WebProject> {
        self.get_optional(name).await?.ok_or(HbError::NotFound)
    }

    pub async fn get_optional(&self, name: &str) -> HbResult<Option<WebProject>> {
        let mut response = self
            .db
            .inner()
            .query("SELECT * OMIT id FROM type::record('_web_projects', $name)")
            .bind(("name", name.to_owned()))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let records: Vec<serde_json::Value> = response.take(0).map_err(database_error)?;
        records
            .into_iter()
            .next()
            .map(|value| serde_json::from_value(value).map_err(database_error))
            .transpose()
    }

    pub async fn save(&self, project: &WebProject) -> HbResult<()> {
        let value = serde_json::to_value(project).map_err(database_error)?;
        self.db
            .inner()
            .query("UPSERT ONLY type::record('_web_projects', $name) CONTENT $project")
            .bind(("name", project.name.clone()))
            .bind(("project", value))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        Ok(())
    }

    pub async fn delete(&self, name: &str) -> HbResult<()> {
        self.get(name).await?;
        self.db
            .inner()
            .query("DELETE ONLY type::record('_web_projects', $name)")
            .bind(("name", name.to_owned()))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        Ok(())
    }
}
