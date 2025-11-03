use std::{collections::HashMap, sync::Arc};

use anyhow::{format_err, Result};
use async_trait::async_trait;
use deadpool_postgres::Pool;
#[cfg(test)]
use mockall::automock;
use tokio_postgres::types::Json;

use crate::registrar::{DataSource, DtConfig, Foundation, Project};

/// Type alias to represent a DB trait object.
pub(crate) type DynDB = Arc<dyn DB + Send + Sync>;

/// Trait that defines some operations a DB implementation must support.
#[async_trait]
#[cfg_attr(test, automock)]
pub(crate) trait DB {
    /// Get foundations registered in the database.
    async fn foundations(&self) -> Result<Vec<Foundation>>;

    /// Get projects for the foundation provided.
    async fn foundation_projects(
        &self,
        foundation_id: &str,
    ) -> Result<HashMap<String, Option<String>>>;

    /// Register project provided in the database.
    async fn register_project(&self, foundation_id: &str, project: &Project) -> Result<()>;

    /// Unregister project provided from the database.
    async fn unregister_project(&self, foundation_id: &str, project_name: &str) -> Result<()>;
}

/// DB implementation backed by PostgreSQL.
pub(crate) struct PgDB {
    pool: Pool,
}

impl PgDB {
    /// Create a new PgDB instance.
    pub(crate) fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DB for PgDB {
    /// #[DB::foundations]
    async fn foundations(&self) -> Result<Vec<Foundation>> {
        let db = self.pool.get().await?;
        let foundations = db
            .query(
                "SELECT foundation_id, data_source_type, data_source_config FROM foundation",
                &[],
            )
            .await?
            .iter()
            .map(|row| -> Result<Foundation> {
                let foundation_id: String = row.get("foundation_id");
                let data_source_type: String = row.get("data_source_type");
                let data_source_config: serde_json::Value = row.get("data_source_config");

                let data_source = match data_source_type.as_str() {
                    "yaml_url" => {
                        let data_url = data_source_config["data_url"]
                            .as_str()
                            .ok_or_else(|| format_err!("Missing data_url in config"))?
                            .to_string();
                        DataSource::YamlUrl { data_url }
                    }
                    "dependency_track" => {
                        let dt_config: DtConfig = serde_json::from_value(data_source_config)?;
                        DataSource::DependencyTrack(dt_config)
                    }
                    _ => {
                        return Err(format_err!(
                            "Unknown data source type: {}",
                            data_source_type
                        ))
                    }
                };

                Ok(Foundation {
                    foundation_id,
                    data_source,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(foundations)
    }

    /// #[DB::foundation_projects]
    async fn foundation_projects(
        &self,
        foundation_id: &str,
    ) -> Result<HashMap<String, Option<String>>> {
        let db = self.pool.get().await?;
        let projects = db
            .query(
                "select name, digest from project where foundation_id = $1::text",
                &[&foundation_id],
            )
            .await?
            .iter()
            .map(|row| (row.get("name"), row.get("digest")))
            .collect();
        Ok(projects)
    }

    /// #[DB::register_project]
    async fn register_project(&self, foundation_id: &str, project: &Project) -> Result<()> {
        let db = self.pool.get().await?;
        db.execute(
            "select register_project($1::text, $2::jsonb)",
            &[&foundation_id, &Json(project)],
        )
        .await?;
        Ok(())
    }

    /// #[DB::unregister_project]
    async fn unregister_project(&self, foundation_id: &str, project_name: &str) -> Result<()> {
        let db = self.pool.get().await?;
        db.execute(
            "select unregister_project($1::text, $2::text)",
            &[&foundation_id, &project_name],
        )
        .await?;
        Ok(())
    }
}
