use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Error, Result, format_err};
use config::Config;
use futures::stream::{self, StreamExt};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::time::{Instant, timeout};
use tracing::{debug, error, info, instrument};

use crate::db::DynDB;
use crate::dt_client::{DtClient, DtHttpClient};
use crate::dt_mapper::{extract_repository_url_with_lookup, should_process_component};

/// Maximum time that can take processing a foundation data file.
const FOUNDATION_TIMEOUT: u64 = 300;

/// Configuration for connecting to a Dependency-Track instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DtConfig {
    pub dt_url: String,
    pub dt_api_key: String,
}

/// Data source for a foundation - either a YAML URL or a Dependency-Track instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum DataSource {
    YamlUrl { data_url: String },
    DependencyTrack(DtConfig),
}

/// Represents a foundation registered in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Foundation {
    pub foundation_id: String,
    #[serde(flatten)]
    pub data_source: DataSource,
}

/// Represents a project to be registered or updated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Project {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    pub description: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_dark_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub devstats_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_at: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub maturity: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,

    pub repositories: Vec<Repository>,
}

impl Project {
    pub(crate) fn set_digest(&mut self) -> Result<()> {
        let data = bincode::serde::encode_to_vec(&self, bincode::config::legacy())?;
        let digest = hex::encode(Sha256::digest(data));
        self.digest = Some(digest);
        Ok(())
    }
}

/// Represents a project's repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Repository {
    pub name: String,
    pub url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_sets: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
}

/// Represents a component that could not be mapped to a repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UnmappedComponent {
    pub component_uuid: String,
    pub component_name: String,
    pub component_version: Option<String>,
    pub component_group: Option<String>,
    pub purl: Option<String>,
    pub component_type: String,
    pub classifier: String,
    pub external_references: Option<serde_json::Value>,
    pub mapping_notes: String,
}

/// Result of attempting to find a repository URL for a component.
#[derive(Debug, Clone)]
pub(crate) enum RepositoryLookupResult {
    /// Repository URL was found (either from DB mapping or auto-discovery)
    Found(String),
    /// Repository URL could not be found, component should be stored as unmapped
    NotFound(UnmappedComponent),
}

/// Process foundations registered in the database.
#[instrument(skip_all, err)]
pub(crate) async fn run(cfg: &Config, db: DynDB) -> Result<()> {
    info!("started");

    // Process foundations
    let http_client = reqwest::Client::new();
    let foundations = db.foundations().await?;
    #[allow(clippy::manual_try_fold)]
    let result = stream::iter(foundations)
        .map(|foundation| async {
            let foundation_id = foundation.foundation_id.clone();
            match timeout(
                Duration::from_secs(FOUNDATION_TIMEOUT),
                process_foundation(db.clone(), http_client.clone(), foundation),
            )
            .await
            {
                Ok(result) => result,
                Err(err) => Err(format_err!("{err}")),
            }
            .context(format!(
                "error processing foundation {foundation_id} data file",
            ))
        })
        .buffer_unordered(cfg.get("registrar.concurrency")?)
        .collect::<Vec<Result<()>>>()
        .await
        .into_iter()
        .fold(
            Ok::<(), Error>(()),
            |final_result, task_result| match task_result {
                Ok(()) => final_result,
                Err(task_err) => match final_result {
                    Ok(()) => Err(Into::into(task_err)),
                    Err(final_err) => Err(format_err!("{final_err:#}\n{task_err:#}")),
                },
            },
        );

    info!("finished");
    result
}

/// Process foundation based on its data source type (YAML URL or Dependency-Track).
#[instrument(fields(foundation = foundation.foundation_id), skip_all, err)]
async fn process_foundation(
    db: DynDB,
    http_client: reqwest::Client,
    foundation: Foundation,
) -> Result<()> {
    match foundation.data_source.clone() {
        DataSource::YamlUrl { data_url } => {
            process_yaml_foundation(db, http_client, foundation, &data_url).await
        }
        DataSource::DependencyTrack(_) => process_dt_foundation(db, foundation).await,
    }
}

/// Process foundation's YAML data file. New projects available will be registered
/// in the database and existing ones which have changed will be updated. When
/// a project is removed from the data file, it'll be removed from the database
/// as well.
#[instrument(fields(foundation = foundation.foundation_id), skip_all, err)]
async fn process_yaml_foundation(
    db: DynDB,
    http_client: reqwest::Client,
    foundation: Foundation,
    data_url: &str,
) -> Result<()> {
    let start = Instant::now();
    debug!("started (YAML)");

    // Fetch foundation data file
    let resp = http_client.get(data_url).send().await?;
    if resp.status() != StatusCode::OK {
        return Err(format_err!(
            "unexpected status code getting data file: {}",
            resp.status()
        ));
    }
    let data = resp.text().await?;

    // Get projects available in the data file
    let tmp: Vec<Project> = serde_yaml::from_str(&data)?;
    let mut projects_available: HashMap<String, Project> = HashMap::with_capacity(tmp.len());
    for mut project in tmp {
        // Do not include repositories that have been excluded for this service
        project.repositories.retain(|r| {
            if let Some(exclude) = &r.exclude {
                return !exclude.contains(&"clomonitor".to_string());
            }
            true
        });

        project.set_digest()?;
        projects_available.insert(project.name.clone(), project);
    }

    // Get projects registered in the database
    let foundation_id = &foundation.foundation_id;
    let projects_registered = db.foundation_projects(foundation_id).await?;

    // Register or update available projects as needed
    for (name, project) in &projects_available {
        // Check if the project is already registered
        if let Some(registered_digest) = projects_registered.get(name) {
            if registered_digest == &project.digest {
                continue;
            }
        }

        // Register project
        debug!(project = project.name, "registering");
        if let Err(err) = db.register_project(foundation_id, project).await {
            error!(?err, project = project.name, "error registering");
        }
    }

    // Unregister projects no longer available in the data file
    if !projects_available.is_empty() {
        for name in projects_registered.keys() {
            if !projects_available.contains_key(name) {
                debug!(project = name, "unregistering");
                if let Err(err) = db.unregister_project(foundation_id, name).await {
                    error!(?err, project = name, "error unregistering");
                }
            }
        }
    }

    debug!(duration_secs = start.elapsed().as_secs(), "completed");
    Ok(())
}

/// Process foundation's Dependency-Track instance. Fetches all projects and
/// components from DT, converts them to CLOMonitor projects, and registers
/// them in the database.
#[instrument(fields(foundation = foundation.foundation_id), skip_all, err)]
async fn process_dt_foundation(db: DynDB, foundation: Foundation) -> Result<()> {
    let start = Instant::now();
    debug!("started (Dependency-Track)");

    let dt_config = match &foundation.data_source {
        DataSource::DependencyTrack(config) => config,
        _ => return Err(format_err!("Expected DependencyTrack data source")),
    };

    // Create DT client
    let dt_client = DtHttpClient::new(dt_config.dt_url.clone(), dt_config.dt_api_key.clone());

    // Fetch all projects from DT
    let dt_projects = dt_client.get_projects().await?;
    debug!("Found {} DT projects", dt_projects.len());

    // Collect all components from all projects and convert to CLOMonitor projects
    let mut all_projects_to_register = HashMap::new();
    let mut unmapped_components = Vec::new();
    let mut mapped_count = 0;
    let mut unmapped_count = 0;

    for dt_project in dt_projects {
        debug!(
            "Processing DT project: {} ({})",
            dt_project.name, dt_project.uuid
        );

        let components = dt_client.get_project_components(&dt_project.uuid).await?;
        debug!(
            "Found {} components in project {}",
            components.len(),
            dt_project.name
        );

        for component in components {
            // Filter components
            if !should_process_component(&component) {
                debug!(
                    "Skipping component {} with classifier {}",
                    component.name, component.classifier
                );
                continue;
            }

            // Try to find repository URL with DB lookup
            match extract_repository_url_with_lookup(&component, &db).await {
                RepositoryLookupResult::Found(repo_url) => {
                    // Create CLOMonitor project with the found repository URL
                    match build_project_from_component(&component, &dt_project.name, repo_url) {
                        Ok(project) => {
                            all_projects_to_register.insert(project.name.clone(), project);
                            mapped_count += 1;
                        }
                        Err(e) => {
                            error!("Failed to build project for component {}: {}", component.name, e);
                        }
                    }
                }
                RepositoryLookupResult::NotFound(unmapped) => {
                    debug!("No repository URL found for component {}", component.name);
                    unmapped_components.push(unmapped);
                    unmapped_count += 1;
                }
            }
        }
    }

    info!(
        "DT foundation {}: {} components mapped, {} unmapped, {} total projects",
        foundation.foundation_id,
        mapped_count,
        unmapped_count,
        all_projects_to_register.len()
    );

    // Store all unmapped components for later review
    for unmapped in unmapped_components {
        if let Err(e) = db.save_unmapped_component(&foundation.foundation_id, &unmapped).await {
            error!("Failed to save unmapped component {}: {}", unmapped.component_name, e);
        }
    }

    // Get currently registered projects
    let foundation_id = &foundation.foundation_id;
    let projects_registered = db.foundation_projects(foundation_id).await?;

    // Register or update projects
    for (name, project) in &all_projects_to_register {
        // Check if the project is already registered
        if let Some(registered_digest) = projects_registered.get(name) {
            if registered_digest == &project.digest {
                continue;
            }
        }

        debug!(project = project.name, "registering");
        if let Err(err) = db.register_project(foundation_id, project).await {
            error!(?err, project = project.name, "error registering");
        }
    }

    // Unregister projects no longer in DT
    if !all_projects_to_register.is_empty() {
        for name in projects_registered.keys() {
            if !all_projects_to_register.contains_key(name) {
                debug!(project = name, "unregistering");
                if let Err(err) = db.unregister_project(foundation_id, name).await {
                    error!(?err, project = name, "error unregistering");
                }
            }
        }
    }

    debug!(duration_secs = start.elapsed().as_secs(), "completed");
    Ok(())
}

/// Extract GitHub organization/user avatar URL from repository URL
fn extract_github_logo_url(repo_url: &str) -> Option<String> {
    // Only process GitHub URLs
    if !repo_url.starts_with("https://github.com/") {
        return None;
    }

    // Extract owner from URL: https://github.com/owner/repo -> owner
    let path = repo_url.trim_start_matches("https://github.com/");
    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() >= 2 {
        let owner = parts[0];
        // GitHub provides organization/user avatars at predictable URLs
        // We use size 200 for consistent sizing
        Some(format!("https://github.com/{}.png?size=200", owner))
    } else {
        None
    }
}

/// Build a CLOMonitor project from a DT component with a known repository URL.
fn build_project_from_component(
    component: &crate::dt_types::DtComponent,
    dt_project_name: &str,
    repo_url: String,
) -> Result<Project> {
    // Format component name
    let parts = vec![
        component.group.as_deref(),
        Some(component.name.as_str()),
        component.version.as_deref(),
    ];
    let name = parts
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
        .replace('/', "-")
        .replace('@', "-")
        .replace(':', "-");

    // Format display name
    let display_name = match (&component.group, &component.version) {
        (Some(group), Some(version)) => format!("{}/{} {}", group, component.name, version),
        (Some(group), None) => format!("{}/{}", group, component.name),
        (None, Some(version)) => format!("{} {}", component.name, version),
        (None, None) => component.name.clone(),
    };

    let description = component.description.clone().unwrap_or_else(|| {
        format!(
            "Component from Dependency-Track project: {}",
            dt_project_name
        )
    });

    // Extract logo URL from GitHub repository (before repo_url is moved)
    let logo_url = extract_github_logo_url(&repo_url);

    let repository = Repository {
        name: name.clone(),
        url: repo_url,
        check_sets: Some(vec!["code".to_string()]),
        exclude: None,
    };

    let mut project = Project {
        name,
        display_name: Some(display_name),
        description,
        category: Some("library".to_string()),
        home_url: None,
        logo_url,
        logo_dark_url: None,
        devstats_url: None,
        accepted_at: None,
        maturity: None,
        digest: None,
        repositories: vec![repository],
    };

    project.set_digest()?;
    Ok(project)
}

#[cfg(test)]
mod tests {
    use futures::future;
    use mockall::predicate::eq;
    use std::sync::Arc;

    use crate::db::MockDB;

    use super::*;

    const TESTDATA_PATH: &str = "src/testdata";
    const FOUNDATION: &str = "cncf";
    const FAKE_ERROR: &str = "fake error";

    #[tokio::test]
    async fn error_getting_foundations() {
        let cfg = setup_test_config();

        let mut db = MockDB::new();
        db.expect_foundations()
            .times(1)
            .returning(|| Box::pin(future::ready(Err(format_err!(FAKE_ERROR)))));

        let result = run(&cfg, Arc::new(db)).await;
        assert_eq!(result.unwrap_err().root_cause().to_string(), FAKE_ERROR);
    }

    #[tokio::test]
    async fn no_foundations_found() {
        let cfg = setup_test_config();

        let mut db = MockDB::new();
        db.expect_foundations()
            .times(1)
            .returning(|| Box::pin(future::ready(Ok(vec![]))));

        run(&cfg, Arc::new(db)).await.unwrap();
    }

    #[tokio::test]
    async fn error_fetching_foundation_data_file() {
        let cfg = setup_test_config();

        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mut db = MockDB::new();
        db.expect_foundations().times(1).returning(move || {
            Box::pin(future::ready(Ok(vec![Foundation {
                foundation_id: FOUNDATION.to_string(),
                data_source: DataSource::YamlUrl {
                    data_url: url.clone(),
                },
            }])))
        });

        let data_file_req = server
            .mock("GET", "/")
            .with_status(404)
            .create_async()
            .await;

        let result = run(&cfg, Arc::new(db)).await;
        assert_eq!(
            result.unwrap_err().root_cause().to_string(),
            "unexpected status code getting data file: 404 Not Found"
        );
        data_file_req.assert_async().await;
    }

    #[tokio::test]
    async fn invalid_foundation_data_file() {
        let cfg = setup_test_config();

        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mut db = MockDB::new();
        db.expect_foundations().times(1).returning(move || {
            Box::pin(future::ready(Ok(vec![Foundation {
                foundation_id: FOUNDATION.to_string(),
                data_source: DataSource::YamlUrl {
                    data_url: url.clone(),
                },
            }])))
        });

        let data_file_req = server
            .mock("GET", "/")
            .with_status(200)
            .with_body("{invalid")
            .create_async()
            .await;

        let result = run(&cfg, Arc::new(db)).await;
        assert_eq!(
            result.unwrap_err().root_cause().to_string(),
            "invalid type: map, expected a sequence"
        );
        data_file_req.assert_async().await;
    }

    #[tokio::test]
    async fn error_getting_projects_registered_in_database() {
        let cfg = setup_test_config();

        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mut db = MockDB::new();
        db.expect_foundations().times(1).returning(move || {
            Box::pin(future::ready(Ok(vec![Foundation {
                foundation_id: FOUNDATION.to_string(),
                data_source: DataSource::YamlUrl {
                    data_url: url.clone(),
                },
            }])))
        });
        db.expect_foundation_projects()
            .with(eq(FOUNDATION))
            .times(1)
            .returning(|_| Box::pin(future::ready(Err(format_err!(FAKE_ERROR)))));

        let data_file_req = server
            .mock("GET", "/")
            .with_status(200)
            .with_body("")
            .create_async()
            .await;

        let result = run(&cfg, Arc::new(db)).await;
        assert_eq!(result.unwrap_err().root_cause().to_string(), FAKE_ERROR);
        data_file_req.assert_async().await;
    }

    #[tokio::test]
    async fn no_need_to_register_registered_project_same_digest() {
        let cfg = setup_test_config();

        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mut db = MockDB::new();
        db.expect_foundations().times(1).returning(move || {
            Box::pin(future::ready(Ok(vec![Foundation {
                foundation_id: FOUNDATION.to_string(),
                data_source: DataSource::YamlUrl {
                    data_url: url.clone(),
                },
            }])))
        });
        db.expect_foundation_projects()
            .with(eq(FOUNDATION))
            .times(1)
            .returning(|_| {
                let mut projects_registered = HashMap::new();
                projects_registered.insert(
                    "artifact-hub".to_string(),
                    Some(
                        "c5ad3114e4e2c11afa4d981180954c63b71e5282890007d0d475d38278082dd1"
                            .to_string(),
                    ),
                );
                Box::pin(future::ready(Ok(projects_registered)))
            });

        let data_file_req = server
            .mock("GET", "/")
            .with_status(200)
            .with_body_from_file(format!("{TESTDATA_PATH}/cncf.yaml"))
            .create_async()
            .await;

        run(&cfg, Arc::new(db)).await.unwrap();
        data_file_req.assert_async().await;
    }

    #[tokio::test]
    async fn register_project_not_registered_yet() {
        let cfg = setup_test_config();

        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mut db = MockDB::new();
        db.expect_foundations().times(1).returning(move || {
            Box::pin(future::ready(Ok(vec![Foundation {
                foundation_id: FOUNDATION.to_string(),
                data_source: DataSource::YamlUrl {
                    data_url: url.clone(),
                },
            }])))
        });
        db.expect_foundation_projects()
            .with(eq(FOUNDATION))
            .times(1)
            .returning(|_| Box::pin(future::ready(Ok(HashMap::new()))));
        db.expect_register_project()
            .with(
                eq(FOUNDATION),
                eq(Project {
                    name: "artifact-hub".to_string(),
                    display_name: Some("Artifact Hub".to_string()),
                    description: "Artifact Hub is a web-based application that enables finding, installing, and publishing packages and configurations for CNCF projects".to_string(),
                    category: Some("app definition".to_string()),
                    home_url: None,
                    logo_url: Some("https://raw.githubusercontent.com/cncf/artwork/master/projects/artifacthub/icon/color/artifacthub-icon-color.svg".to_string()),
                    logo_dark_url: None,
                    devstats_url: Some("https://artifacthub.devstats.cncf.io/".to_string()),
                    accepted_at: Some("2020-06-23".to_string()),
                    maturity: Some("sandbox".to_string()),
                    digest: Some("c5ad3114e4e2c11afa4d981180954c63b71e5282890007d0d475d38278082dd1".to_string()),
                    repositories: vec![Repository{
                        name: "artifact-hub".to_string(),
                        url: "https://github.com/artifacthub/hub".to_string(),
                        check_sets: Some(vec!["community".to_string(), "code".to_string()]),
                        exclude: None,
                    }]
                }),
            )
            .times(1)
            .returning(|_, _| Box::pin(future::ready(Ok(()))));

        let data_file_req = server
            .mock("GET", "/")
            .with_status(200)
            .with_body_from_file(format!("{TESTDATA_PATH}/cncf.yaml"))
            .create_async()
            .await;

        run(&cfg, Arc::new(db)).await.unwrap();
        data_file_req.assert_async().await;
    }

    #[tokio::test]
    async fn unregister_registered_project() {
        let cfg = setup_test_config();

        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mut db = MockDB::new();
        db.expect_foundations().times(1).returning(move || {
            Box::pin(future::ready(Ok(vec![Foundation {
                foundation_id: FOUNDATION.to_string(),
                data_source: DataSource::YamlUrl {
                    data_url: url.clone(),
                },
            }])))
        });
        db.expect_foundation_projects()
            .with(eq(FOUNDATION))
            .times(1)
            .returning(|_| {
                let mut projects_registered = HashMap::new();
                projects_registered.insert(
                    "artifact-hub".to_string(),
                    Some(
                        "c5ad3114e4e2c11afa4d981180954c63b71e5282890007d0d475d38278082dd1"
                            .to_string(),
                    ),
                );
                projects_registered.insert("project-name".to_string(), Some("digest".to_string()));
                Box::pin(future::ready(Ok(projects_registered)))
            });
        db.expect_unregister_project()
            .with(eq(FOUNDATION), eq("project-name"))
            .times(1)
            .returning(|_, _| Box::pin(future::ready(Ok(()))));

        let data_file_req = server
            .mock("GET", "/")
            .with_status(200)
            .with_body_from_file(format!("{TESTDATA_PATH}/cncf.yaml"))
            .create_async()
            .await;

        run(&cfg, Arc::new(db)).await.unwrap();
        data_file_req.assert_async().await;
    }

    fn setup_test_config() -> Config {
        Config::builder()
            .set_default("registrar.concurrency", 1)
            .unwrap()
            .build()
            .unwrap()
    }

    #[test]
    fn test_dt_config_deserialization() {
        let yaml = r#"
            dt_url: "https://dtrack.example.com"
            dt_api_key: "secret123"
        "#;
        let config: DtConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.dt_url, "https://dtrack.example.com");
        assert_eq!(config.dt_api_key, "secret123");
    }

    #[test]
    fn test_foundation_yaml_url_deserialization() {
        let json = r#"{
            "foundation_id": "cncf",
            "data_url": "https://example.com/data.yaml"
        }"#;
        let foundation: Foundation = serde_json::from_str(json).unwrap();
        assert_eq!(foundation.foundation_id, "cncf");
        assert!(matches!(foundation.data_source, DataSource::YamlUrl { .. }));
        if let DataSource::YamlUrl { data_url } = foundation.data_source {
            assert_eq!(data_url, "https://example.com/data.yaml");
        }
    }

    #[test]
    fn test_foundation_dt_deserialization() {
        let json = r#"{
            "foundation_id": "dt-instance",
            "dt_url": "https://dtrack.example.com",
            "dt_api_key": "secret"
        }"#;
        let foundation: Foundation = serde_json::from_str(json).unwrap();
        assert_eq!(foundation.foundation_id, "dt-instance");
        assert!(matches!(
            foundation.data_source,
            DataSource::DependencyTrack(_)
        ));
        if let DataSource::DependencyTrack(dt_config) = foundation.data_source {
            assert_eq!(dt_config.dt_url, "https://dtrack.example.com");
            assert_eq!(dt_config.dt_api_key, "secret");
        }
    }

    #[tokio::test]
    async fn test_process_dt_foundation() {
        let mut server = mockito::Server::new_async().await;

        // Mock projects endpoint
        let projects_mock = server
            .mock("GET", "/api/v1/project")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body(
                r#"[{"uuid":"proj-1","name":"my-app","description":"Test app","version":"1.0"}]"#,
            )
            .create_async()
            .await;

        // Mock components endpoint
        let components_mock = server
            .mock("GET", "/api/v1/component/project/proj-1")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body(
                r#"[{
                    "uuid":"comp-1",
                    "name":"lodash",
                    "version":"4.17.21",
                    "group":"npm",
                    "classifier":"LIBRARY",
                    "description":"Lodash library",
                    "externalReferences":[{"type":"vcs","url":"https://github.com/lodash/lodash"}]
                }]"#,
            )
            .create_async()
            .await;

        let mut db = MockDB::new();
        db.expect_foundation_projects()
            .with(eq("dt-test"))
            .times(1)
            .returning(|_| Box::pin(future::ready(Ok(HashMap::new()))));

        db.expect_register_project()
            .with(eq("dt-test"), mockall::predicate::function(|p: &Project| {
                p.name == "npm-lodash-4.17.21"
            }))
            .times(1)
            .returning(|_, _| Box::pin(future::ready(Ok(()))));

        let foundation = Foundation {
            foundation_id: "dt-test".to_string(),
            data_source: DataSource::DependencyTrack(DtConfig {
                dt_url: server.url(),
                dt_api_key: "test-key".to_string(),
            }),
        };

        process_dt_foundation(Arc::new(db), foundation)
            .await
            .unwrap();

        projects_mock.assert_async().await;
        components_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_process_dt_with_no_projects() {
        let mut server = mockito::Server::new_async().await;

        // Mock empty projects list
        let projects_mock = server
            .mock("GET", "/api/v1/project")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "0")
            .with_body("[]")
            .create_async()
            .await;

        let mut db = MockDB::new();
        db.expect_foundation_projects()
            .with(eq("dt-test"))
            .times(1)
            .returning(|_| Box::pin(future::ready(Ok(HashMap::new()))));

        let foundation = Foundation {
            foundation_id: "dt-test".to_string(),
            data_source: DataSource::DependencyTrack(DtConfig {
                dt_url: server.url(),
                dt_api_key: "test-key".to_string(),
            }),
        };

        // Should succeed without errors
        process_dt_foundation(Arc::new(db), foundation)
            .await
            .unwrap();

        projects_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_process_dt_project_with_no_components() {
        let mut server = mockito::Server::new_async().await;

        // Mock project with no components
        let projects_mock = server
            .mock("GET", "/api/v1/project")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body(r#"[{"uuid":"proj-1","name":"empty-project","version":"1.0"}]"#)
            .create_async()
            .await;

        let components_mock = server
            .mock("GET", "/api/v1/component/project/proj-1")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "0")
            .with_body("[]")
            .create_async()
            .await;

        let mut db = MockDB::new();
        db.expect_foundation_projects()
            .with(eq("dt-test"))
            .times(1)
            .returning(|_| Box::pin(future::ready(Ok(HashMap::new()))));

        // No register_project should be called since no components

        let foundation = Foundation {
            foundation_id: "dt-test".to_string(),
            data_source: DataSource::DependencyTrack(DtConfig {
                dt_url: server.url(),
                dt_api_key: "test-key".to_string(),
            }),
        };

        // Should succeed without errors
        process_dt_foundation(Arc::new(db), foundation)
            .await
            .unwrap();

        projects_mock.assert_async().await;
        components_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_skip_components_without_repo_urls() {
        let mut server = mockito::Server::new_async().await;

        let projects_mock = server
            .mock("GET", "/api/v1/project")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body(r#"[{"uuid":"proj-1","name":"test-project","version":"1.0"}]"#)
            .create_async()
            .await;

        // Component without externalReferences and without purl
        let components_mock = server
            .mock("GET", "/api/v1/component/project/proj-1")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body(
                r#"[{
                    "uuid":"comp-1",
                    "name":"no-repo",
                    "version":"1.0",
                    "classifier":"LIBRARY"
                }]"#,
            )
            .create_async()
            .await;

        let mut db = MockDB::new();

        // Component has no purl, so get_component_mapping won't be called
        // But save_unmapped_component WILL be called since component has no repo URL
        db.expect_save_unmapped_component()
            .with(
                eq("dt-test"),
                mockall::predicate::function(|unmapped: &UnmappedComponent| {
                    unmapped.component_name == "no-repo"
                        && unmapped.component_version == Some("1.0".to_string())
                        && unmapped.classifier == "LIBRARY"
                }),
            )
            .times(1)
            .returning(|_, _| Box::pin(future::ready(Ok(()))));

        db.expect_foundation_projects()
            .with(eq("dt-test"))
            .times(1)
            .returning(|_| Box::pin(future::ready(Ok(HashMap::new()))));

        // No register_project should be called since component has no repo URL

        let foundation = Foundation {
            foundation_id: "dt-test".to_string(),
            data_source: DataSource::DependencyTrack(DtConfig {
                dt_url: server.url(),
                dt_api_key: "test-key".to_string(),
            }),
        };

        // Should succeed, just skips the component
        process_dt_foundation(Arc::new(db), foundation)
            .await
            .unwrap();

        projects_mock.assert_async().await;
        components_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_skip_non_library_components() {
        let mut server = mockito::Server::new_async().await;

        let projects_mock = server
            .mock("GET", "/api/v1/project")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body(r#"[{"uuid":"proj-1","name":"test-project","version":"1.0"}]"#)
            .create_async()
            .await;

        // Components with CONTAINER and APPLICATION classifiers (which are filtered)
        // Note: FRAMEWORK and LIBRARY classifiers are NOT filtered
        let components_mock = server
            .mock("GET", "/api/v1/component/project/proj-1")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "2")
            .with_body(
                r#"[
                    {
                        "uuid":"comp-1",
                        "name":"container-app",
                        "version":"1.0",
                        "classifier":"CONTAINER",
                        "externalReferences":[{"type":"vcs","url":"https://github.com/test/container"}]
                    },
                    {
                        "uuid":"comp-2",
                        "name":"application",
                        "version":"1.0",
                        "classifier":"APPLICATION",
                        "externalReferences":[{"type":"vcs","url":"https://github.com/test/app"}]
                    }
                ]"#,
            )
            .create_async()
            .await;

        let mut db = MockDB::new();
        db.expect_foundation_projects()
            .with(eq("dt-test"))
            .times(1)
            .returning(|_| Box::pin(future::ready(Ok(HashMap::new()))));

        // No register_project should be called since all components (CONTAINER, APPLICATION) are filtered out

        let foundation = Foundation {
            foundation_id: "dt-test".to_string(),
            data_source: DataSource::DependencyTrack(DtConfig {
                dt_url: server.url(),
                dt_api_key: "test-key".to_string(),
            }),
        };

        // Should succeed, just skips CONTAINER and APPLICATION components
        process_dt_foundation(Arc::new(db), foundation)
            .await
            .unwrap();

        projects_mock.assert_async().await;
        components_mock.assert_async().await;
    }
}
