use std::{collections::HashMap, sync::Arc};

use anyhow::{Result, format_err};
use async_trait::async_trait;
use deadpool_postgres::Pool;
#[cfg(test)]
use mockall::automock;
use tokio_postgres::types::Json;

use crate::registrar::{
    DataSource, DtConfig, Foundation, ImporterApiConfig, Project, UnmappedComponent,
};

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

    /// Get repository URL mapping for a package URL (purl).
    async fn get_component_mapping(&self, purl: &str) -> Result<Option<String>>;

    /// Save a component mapping to the database.
    async fn save_component_mapping(
        &self,
        purl: &str,
        repo_url: &str,
        source: &str,
        confidence: Option<i16>,
    ) -> Result<()>;

    /// Save an unmapped component to the database.
    async fn save_unmapped_component(
        &self,
        foundation_id: &str,
        component: &UnmappedComponent,
    ) -> Result<()>;
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
                    "importer_api" => {
                        let importer_config: ImporterApiConfig =
                            serde_json::from_value(data_source_config)?;
                        DataSource::ImporterApi(importer_config)
                    }
                    _ => {
                        return Err(format_err!(
                            "Unknown data source type: {}",
                            data_source_type
                        ));
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

    /// #[DB::get_component_mapping]
    async fn get_component_mapping(&self, purl: &str) -> Result<Option<String>> {
        let db = self.pool.get().await?;
        let row = db
            .query_opt(
                "SELECT repository_url FROM dt_component_mapping WHERE purl = $1",
                &[&purl],
            )
            .await?;
        Ok(row.map(|r| r.get("repository_url")))
    }

    /// #[DB::save_component_mapping]
    async fn save_component_mapping(
        &self,
        purl: &str,
        repo_url: &str,
        source: &str,
        confidence: Option<i16>,
    ) -> Result<()> {
        let db = self.pool.get().await?;
        db.execute(
            "INSERT INTO dt_component_mapping
             (purl, repository_url, mapping_source, confidence_score)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (purl) DO UPDATE
             SET repository_url = EXCLUDED.repository_url,
                 mapping_source = EXCLUDED.mapping_source,
                 confidence_score = EXCLUDED.confidence_score,
                 updated_at = NOW()",
            &[&purl, &repo_url, &source, &confidence],
        )
        .await?;
        Ok(())
    }

    /// #[DB::save_unmapped_component]
    async fn save_unmapped_component(
        &self,
        foundation_id: &str,
        component: &UnmappedComponent,
    ) -> Result<()> {
        let db = self.pool.get().await?;
        db.execute(
            "INSERT INTO dt_unmapped_components
             (foundation_id, component_uuid, component_name, component_version,
              component_group, purl, component_type, classifier,
              external_references, mapping_notes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (foundation_id, purl)
             WHERE purl IS NOT NULL AND NOT ignored
             DO UPDATE SET
                last_seen = NOW(),
                mapping_attempts = dt_unmapped_components.mapping_attempts + 1",
            &[
                &foundation_id,
                &component.component_uuid,
                &component.component_name,
                &component.component_version,
                &component.component_group,
                &component.purl,
                &component.component_type,
                &component.classifier,
                &Json(&component.external_references),
                &component.mapping_notes,
            ],
        )
        .await?;
        Ok(())
    }
}
