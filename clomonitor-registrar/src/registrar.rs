use std::{collections::{HashMap, HashSet}, time::Duration};

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
use crate::registry_apis::{RegistryRouter, NpmRegistry, maven::MavenRegistry, pypi::PyPIRegistry};

/// Maximum time that can take processing a foundation data file.
const FOUNDATION_TIMEOUT: u64 = 300;

/// Configuration for connecting to a Dependency-Track instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DtConfig {
    pub dt_url: String,
    pub dt_api_key: String,
}

/// Configuration for registry API lookups.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct RegistryApiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub npm_enabled: bool,
    #[serde(default)]
    pub maven_enabled: bool,
    #[serde(default)]
    pub pypi_enabled: bool,
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

    // Read registry API configuration
    let registry_api_cfg = cfg
        .get::<RegistryApiConfig>("registry_apis")
        .unwrap_or_default();
    debug!("Registry API config: {:?}", registry_api_cfg);

    // Process foundations
    let http_client = reqwest::Client::new();
    let foundations = db.foundations().await?;
    #[allow(clippy::manual_try_fold)]
    let result = stream::iter(foundations)
        .map(|foundation| async {
            let foundation_id = foundation.foundation_id.clone();
            match timeout(
                Duration::from_secs(FOUNDATION_TIMEOUT),
                process_foundation(db.clone(), http_client.clone(), foundation, registry_api_cfg.clone()),
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
    registry_api_cfg: RegistryApiConfig,
) -> Result<()> {
    match foundation.data_source.clone() {
        DataSource::YamlUrl { data_url } => {
            process_yaml_foundation(db, http_client, foundation, &data_url).await
        }
        DataSource::DependencyTrack(_) => {
            process_dt_foundation(db, foundation, registry_api_cfg).await
        }
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
async fn process_dt_foundation(
    db: DynDB,
    foundation: Foundation,
    registry_api_cfg: RegistryApiConfig,
) -> Result<()> {
    let start = Instant::now();
    debug!("started (Dependency-Track)");

    let dt_config = match &foundation.data_source {
        DataSource::DependencyTrack(config) => config,
        _ => return Err(format_err!("Expected DependencyTrack data source")),
    };

    // Create registry API router if enabled
    let registry_router = if registry_api_cfg.enabled {
        use std::sync::Arc;

        let npm = if registry_api_cfg.npm_enabled {
            info!("Registry API: npm enabled");
            Some(Arc::new(NpmRegistry::new()) as Arc<dyn crate::registry_apis::RegistryApi>)
        } else {
            None
        };

        let maven = if registry_api_cfg.maven_enabled {
            info!("Registry API: Maven enabled");
            Some(Arc::new(MavenRegistry::new()) as Arc<dyn crate::registry_apis::RegistryApi>)
        } else {
            None
        };

        let pypi = if registry_api_cfg.pypi_enabled {
            info!("Registry API: PyPI enabled");
            Some(Arc::new(PyPIRegistry::new()) as Arc<dyn crate::registry_apis::RegistryApi>)
        } else {
            None
        };

        Some(std::sync::Arc::new(RegistryRouter::new(npm, maven, pypi)))
    } else {
        info!("Registry API: disabled");
        None
    };

    // Create DT client
    let dt_client = DtHttpClient::new(dt_config.dt_url.clone(), dt_config.dt_api_key.clone());

    // Fetch all projects from DT
    let dt_projects = dt_client.get_projects().await?;
    debug!("Found {} DT projects to process", dt_projects.len());

    // Track what we've registered in this run (for cleanup phase)
    let mut registered_in_this_run = HashSet::new();
    let mut stats = ProcessingStats::default();

    // Process each DT project independently with per-project error handling
    for dt_project in dt_projects {
        match process_single_dt_project(
            &db,
            &dt_client,
            &dt_project,
            &foundation.foundation_id,
            registry_router.as_ref(),
        )
        .await
        {
            Ok(result) => {
                stats.merge(&result);
                registered_in_this_run.extend(result.registered_names);
            }
            Err(e) => {
                error!(
                    "Failed to process DT project {} ({}): {}",
                    dt_project.name, dt_project.uuid, e
                );
                stats.errors.push(format!("{}: {}", dt_project.name, e));
                // Continue to next project instead of failing entire run
            }
        }
    }

    info!(
        "DT foundation {}: {} projects processed, {} components mapped, {} unmapped, {} registration failures, {} unmapped save failures, {} project errors",
        foundation.foundation_id,
        stats.projects_processed,
        stats.components_mapped,
        stats.components_unmapped,
        stats.failed_registrations,
        stats.failed_unmapped_saves,
        stats.errors.len()
    );

    // Cleanup projects no longer in DT
    cleanup_removed_projects(&db, &foundation.foundation_id, &registered_in_this_run).await?;

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

/// Result of processing a single DT project with detailed error tracking.
///
/// This structure provides granular visibility into the outcome of processing
/// a single Dependency-Track project, including success and failure counts.
#[allow(dead_code)]
struct ProcessingResult {
    /// Name of the DT project that was processed
    dt_project_name: String,
    /// UUID of the DT project in Dependency-Track
    dt_project_uuid: String,
    /// Names of projects successfully registered in this run
    registered_names: Vec<String>,
    /// Number of components successfully mapped and registered
    mapped_count: usize,
    /// Number of components that could not be mapped to repositories
    unmapped_count: usize,
    /// Number of components that failed to register due to errors
    failed_registrations: usize,
    /// Number of unmapped components that failed to save
    failed_unmapped_saves: usize,
}

/// Aggregated statistics across all DT projects with comprehensive error tracking.
///
/// Tracks both successful operations and failures to provide complete visibility
/// into the registration process. Used for logging and monitoring.
#[derive(Default)]
struct ProcessingStats {
    /// Number of DT projects successfully processed
    projects_processed: usize,
    /// Number of components successfully mapped and registered
    components_mapped: usize,
    /// Number of components that could not be mapped to repositories
    components_unmapped: usize,
    /// Number of registration failures (project creation/update errors)
    failed_registrations: usize,
    /// Number of failures saving unmapped components
    failed_unmapped_saves: usize,
    /// List of project-level errors with context
    errors: Vec<String>,
}

impl ProcessingStats {
    /// Merge results from a single project into aggregate statistics.
    ///
    /// This accumulates counts across all processed DT projects, including
    /// both successes and failures for comprehensive reporting.
    fn merge(&mut self, result: &ProcessingResult) {
        self.projects_processed += 1;
        self.components_mapped += result.mapped_count;
        self.components_unmapped += result.unmapped_count;
        self.failed_registrations += result.failed_registrations;
        self.failed_unmapped_saves += result.failed_unmapped_saves;
    }
}

/// Process a single DT project and immediately register its components.
///
/// This function implements the incremental registration pattern where each component
/// is registered to the database immediately after successful mapping, rather than
/// accumulating all components in memory and registering in a batch.
///
/// # Resilience Strategy
///
/// - **Incremental Progress**: Each successfully mapped component is immediately registered,
///   so progress is preserved even if later components fail.
/// - **Error Isolation**: Component-level errors (mapping failures, registration failures)
///   are logged but don't stop processing of other components.
/// - **Graceful Degradation**: The function continues processing even if some operations
///   fail, returning a detailed result with success and failure counts.
///
/// # Arguments
///
/// * `db` - Database connection for registration and unmapped component storage
/// * `dt_client` - HTTP client for fetching components from Dependency-Track API
/// * `dt_project` - The DT project to process
/// * `foundation_id` - Foundation identifier for registration
/// * `registry_router` - Optional registry API router for npm/Maven/PyPI lookups
///
/// # Returns
///
/// Returns `Ok(ProcessingResult)` with detailed counts of successes and failures,
/// or `Err` only if the component fetch from DT API fails (unrecoverable).
///
/// # Error Handling
///
/// - **DT API errors**: Propagated as `Err` (component fetch failure stops processing)
/// - **Mapping errors**: Logged, component saved as unmapped, processing continues
/// - **Registration errors**: Logged, tracked in `failed_registrations`, processing continues
/// - **Unmapped save errors**: Logged, tracked in `failed_unmapped_saves`, processing continues
///
/// # Examples
///
/// ```ignore
/// let result = process_single_dt_project(
///     &db,
///     &dt_client,
///     &dt_project,
///     "my-foundation",
///     Some(&registry_router),
/// ).await?;
///
/// println!("Mapped: {}, Unmapped: {}, Failed: {}",
///     result.mapped_count,
///     result.unmapped_count,
///     result.failed_registrations);
/// ```
async fn process_single_dt_project(
    db: &DynDB,
    dt_client: &DtHttpClient,
    dt_project: &crate::dt_types::DtProject,
    foundation_id: &str,
    registry_router: Option<&std::sync::Arc<RegistryRouter>>,
) -> Result<ProcessingResult> {
    debug!(
        "Processing DT project: {} ({})",
        dt_project.name, dt_project.uuid
    );

    let components = dt_client.get_project_components(&dt_project.uuid).await?;
    debug!("Found {} components in {}", components.len(), dt_project.name);

    let mut result = ProcessingResult {
        dt_project_name: dt_project.name.clone(),
        dt_project_uuid: dt_project.uuid.clone(),
        registered_names: Vec::new(),
        mapped_count: 0,
        unmapped_count: 0,
        failed_registrations: 0,
        failed_unmapped_saves: 0,
    };

    for component in components {
        // Filter components
        if !should_process_component(&component) {
            debug!(
                "Skipping component {} with classifier {}",
                component.name, component.classifier
            );
            continue;
        }

        // Try to find repository URL
        match extract_repository_url_with_lookup(&component, db, registry_router).await {
            RepositoryLookupResult::Found(repo_url) => {
                // Build CLOMonitor project
                match build_project_from_component(&component, &dt_project.name, repo_url) {
                    Ok(project) => {
                        // Immediate registration (not batched) - errors are tracked but non-fatal
                        match register_project_if_changed(db, foundation_id, &project).await {
                            Ok(registered) => {
                                if registered {
                                    debug!("Registered project: {}", project.name);
                                } else {
                                    debug!("Skipped registration (unchanged): {}", project.name);
                                }
                                result.registered_names.push(project.name.clone());
                                result.mapped_count += 1;
                            }
                            Err(e) => {
                                error!("Failed to register {}: {}", project.name, e);
                                result.failed_registrations += 1;
                                // Continue to next component - don't let one failure stop others
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to build project for component {}: {}", component.name, e);
                        result.failed_registrations += 1;
                    }
                }
            }
            RepositoryLookupResult::NotFound(unmapped) => {
                // Immediate save (not batched) - errors are tracked but non-fatal
                match db.save_unmapped_component(foundation_id, &unmapped).await {
                    Ok(_) => {
                        result.unmapped_count += 1;
                    }
                    Err(e) => {
                        error!("Failed to save unmapped component {}: {}", unmapped.component_name, e);
                        result.failed_unmapped_saves += 1;
                        // Continue to next component - don't let one failure stop others
                    }
                }
            }
        }
    }

    debug!(
        "Completed DT project {}: {} mapped, {} unmapped",
        dt_project.name, result.mapped_count, result.unmapped_count
    );

    Ok(result)
}

/// Register a project only if it doesn't exist or has changed (digest-based deduplication).
///
/// This function implements idempotent registration by comparing project digests
/// before performing the registration. This enables efficient restarts where already-
/// registered projects can be skipped without redundant database writes.
///
/// # Arguments
///
/// * `db` - Database connection
/// * `foundation_id` - Foundation identifier
/// * `project` - Project to register (must have digest set)
///
/// # Returns
///
/// * `Ok(true)` - Project was registered (new or changed)
/// * `Ok(false)` - Project was skipped (already registered with same digest)
/// * `Err` - Database error occurred
///
/// # Error Handling
///
/// - Database query errors propagate as `Err`
/// - Registration errors propagate as `Err`
/// - This function does not swallow errors - caller must handle them
///
/// # Examples
///
/// ```ignore
/// match register_project_if_changed(&db, "foundation-id", &project).await {
///     Ok(true) => println!("Project registered"),
///     Ok(false) => println!("Project unchanged, skipped"),
///     Err(e) => eprintln!("Registration failed: {}", e),
/// }
/// ```
async fn register_project_if_changed(
    db: &DynDB,
    foundation_id: &str,
    project: &Project,
) -> Result<bool> {
    // Check if project already exists with same digest
    let existing_projects = db.foundation_projects(foundation_id).await?;

    if let Some(registered_digest) = existing_projects.get(&project.name)
        && registered_digest == &project.digest
    {
        // Project unchanged, skip registration
        return Ok(false);
    }

    // Project is new or changed, register it
    db.register_project(foundation_id, project).await?;
    Ok(true)
}

/// Cleanup projects that are no longer present in Dependency-Track.
///
/// This function unregisters projects from the database that were previously registered
/// but are no longer found in the current DT processing run. This handles the case where
/// components are removed from DT or no longer match our filtering criteria.
///
/// # Arguments
///
/// * `db` - Database connection
/// * `foundation_id` - Foundation identifier
/// * `registered_in_this_run` - Set of project names that were registered in this run
///
/// # Returns
///
/// * `Ok(())` - Cleanup completed (even if some unregister operations failed)
/// * `Err` - Database query error occurred when fetching registered projects
///
/// # Error Handling
///
/// - Query errors (getting registered projects) propagate as `Err`
/// - Individual unregister errors are logged but don't stop cleanup
/// - Empty `registered_in_this_run` set triggers early return (no cleanup needed)
///
/// # Resilience
///
/// This function is resilient to individual unregister failures - if one project
/// fails to unregister, others will still be attempted. This prevents a single
/// bad project from blocking cleanup of all removed projects.
///
/// # Examples
///
/// ```ignore
/// let mut registered = HashSet::new();
/// registered.insert("project-1".to_string());
/// registered.insert("project-2".to_string());
///
/// // Unregister any projects not in the set
/// cleanup_removed_projects(&db, "foundation-id", &registered).await?;
/// ```
async fn cleanup_removed_projects(
    db: &DynDB,
    foundation_id: &str,
    registered_in_this_run: &HashSet<String>,
) -> Result<()> {
    if registered_in_this_run.is_empty() {
        debug!("No projects registered, skipping cleanup");
        return Ok(());
    }

    let projects_registered = db.foundation_projects(foundation_id).await?;

    for name in projects_registered.keys() {
        if !registered_in_this_run.contains(name) {
            debug!("Unregistering removed project: {}", name);
            if let Err(err) = db.unregister_project(foundation_id, name).await {
                error!(?err, project = name, "error unregistering");
            }
        }
    }

    Ok(())
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
        // Called once in register_project_if_changed and once in cleanup_removed_projects
        db.expect_foundation_projects()
            .with(eq("dt-test"))
            .times(2)
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

        process_dt_foundation(Arc::new(db), foundation, RegistryApiConfig::default())
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

        let db = MockDB::new();
        // No foundation_projects call expected because cleanup is skipped when no projects registered

        let foundation = Foundation {
            foundation_id: "dt-test".to_string(),
            data_source: DataSource::DependencyTrack(DtConfig {
                dt_url: server.url(),
                dt_api_key: "test-key".to_string(),
            }),
        };

        // Should succeed without errors
        process_dt_foundation(Arc::new(db), foundation, RegistryApiConfig::default())
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

        let db = MockDB::new();
        // No foundation_projects call expected because cleanup is skipped when no projects registered
        // No register_project should be called since no components

        let foundation = Foundation {
            foundation_id: "dt-test".to_string(),
            data_source: DataSource::DependencyTrack(DtConfig {
                dt_url: server.url(),
                dt_api_key: "test-key".to_string(),
            }),
        };

        // Should succeed without errors
        process_dt_foundation(Arc::new(db), foundation, RegistryApiConfig::default())
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

        // No foundation_projects call expected because cleanup is skipped when no projects registered
        // No register_project should be called since component has no repo URL

        let foundation = Foundation {
            foundation_id: "dt-test".to_string(),
            data_source: DataSource::DependencyTrack(DtConfig {
                dt_url: server.url(),
                dt_api_key: "test-key".to_string(),
            }),
        };

        // Should succeed, just skips the component
        process_dt_foundation(Arc::new(db), foundation, RegistryApiConfig::default())
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

        let db = MockDB::new();
        // No foundation_projects call expected because cleanup is skipped when no projects registered
        // No register_project should be called since all components (CONTAINER, APPLICATION) are filtered out

        let foundation = Foundation {
            foundation_id: "dt-test".to_string(),
            data_source: DataSource::DependencyTrack(DtConfig {
                dt_url: server.url(),
                dt_api_key: "test-key".to_string(),
            }),
        };

        // Should succeed, just skips CONTAINER and APPLICATION components
        process_dt_foundation(Arc::new(db), foundation, RegistryApiConfig::default())
            .await
            .unwrap();

        projects_mock.assert_async().await;
        components_mock.assert_async().await;
    }
}
