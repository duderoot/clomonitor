use anyhow::{format_err, Result};
use crate::db::DynDB;
use crate::dt_types::DtComponent;
use crate::registrar::{Project, Repository, UnmappedComponent, RepositoryLookupResult};

#[cfg(test)]
use crate::dt_types::ExternalReference;

/// Extract repository URL from DT component with database lookup.
/// This function first checks the component mapping table, then falls back to auto-discovery.
pub(crate) async fn extract_repository_url_with_lookup(
    component: &DtComponent,
    db: &DynDB,
) -> RepositoryLookupResult {
    // Priority 1: Check mapping table if purl exists
    if let Some(purl) = &component.purl {
        if let Ok(Some(repo_url)) = db.get_component_mapping(purl).await {
            return RepositoryLookupResult::Found(repo_url);
        }
    }

    // Priority 2: Try auto-discovery (existing logic)
    if let Some(repo_url) = extract_repository_url(component) {
        // Optionally save to mapping table with 'auto' source
        if let Some(purl) = &component.purl {
            let _ = db.save_component_mapping(purl, &repo_url, "auto", Some(80)).await;
        }
        return RepositoryLookupResult::Found(repo_url);
    }

    // Priority 3: No mapping found, create unmapped component
    let unmapped = UnmappedComponent {
        component_uuid: component.uuid.clone(),
        component_name: component.name.clone(),
        component_version: component.version.clone(),
        component_group: component.group.clone(),
        purl: component.purl.clone(),
        component_type: component.classifier.clone(),
        classifier: component.classifier.clone(),
        external_references: component
            .external_references
            .as_ref()
            .and_then(|refs| serde_json::to_value(refs).ok()),
        mapping_notes: "No VCS reference or purl mapping found".to_string(),
    };

    RepositoryLookupResult::NotFound(unmapped)
}

/// Extract repository URL from DT component
pub(crate) fn extract_repository_url(component: &DtComponent) -> Option<String> {
    // Priority 1: Check externalReferences for vcs type
    if let Some(refs) = &component.external_references {
        for ref_ in refs {
            if ref_.type_ == "vcs" && is_valid_git_url(&ref_.url) {
                return Some(normalize_git_url(&ref_.url));
            }
        }
    }

    // Priority 2: Try to extract from purl
    if let Some(purl) = &component.purl {
        if let Some(url) = extract_url_from_purl(purl) {
            return Some(url);
        }
    }

    None
}

fn is_valid_git_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("git@")
}

fn normalize_git_url(url: &str) -> String {
    let mut normalized = url.trim_end_matches('/').to_string();

    // Normalize http:// to https://
    if normalized.starts_with("http://") {
        normalized = normalized.replace("http://", "https://");
    }

    // Convert git@ URLs to https
    if normalized.starts_with("git@github.com:") {
        normalized = normalized.replace("git@github.com:", "https://github.com/");
    }

    // Clean GitHub URLs: extract base repo URL (owner/repo)
    if normalized.starts_with("https://github.com/") {
        normalized = clean_github_url(&normalized);
    }

    // Clean GitLab URLs: extract base repo URL (owner/repo)
    if normalized.starts_with("https://gitlab.com/") {
        normalized = clean_gitlab_url(&normalized);
    }

    // Remove .git suffix
    if normalized.ends_with(".git") {
        normalized.truncate(normalized.len() - 4);
    }

    normalized
}

/// Clean GitHub URL to extract base repository (owner/repo)
/// Examples:
/// - https://github.com/dropwizard/metrics/metrics-core -> https://github.com/dropwizard/metrics
/// - https://github.com/apache/httpcomponents-core/tree/4.4.16/httpcore -> https://github.com/apache/httpcomponents-core
/// - https://github.com/yarnpkg/yarn/blob/master/packages/lockfile -> https://github.com/yarnpkg/yarn
fn clean_github_url(url: &str) -> String {
    let without_prefix = url.trim_start_matches("https://github.com/");

    // Split by '/' and take first two parts (owner/repo)
    let parts: Vec<&str> = without_prefix.split('/').collect();
    if parts.len() >= 2 {
        // Check if third part is a special path (tree, blob, modules, packages, etc.)
        // If so, only take owner/repo
        if parts.len() > 2 {
            let third_part = parts[2];
            if third_part == "tree"
                || third_part == "blob"
                || third_part == "modules"
                || third_part == "packages"
                || !third_part.contains('.') // Likely a submodule/subpath
            {
                return format!("https://github.com/{}/{}", parts[0], parts[1]);
            }
        }
        format!("https://github.com/{}/{}", parts[0], parts[1])
    } else {
        url.to_string()
    }
}

/// Clean GitLab URL to extract base repository (owner/repo)
fn clean_gitlab_url(url: &str) -> String {
    let without_prefix = url.trim_start_matches("https://gitlab.com/");

    // Split by '/' and take first two parts (owner/repo)
    let parts: Vec<&str> = without_prefix.split('/').collect();
    if parts.len() >= 2 {
        // Check if third part is a special path (tree, blob, etc.)
        if parts.len() > 2 {
            let third_part = parts[2];
            if third_part == "tree" || third_part == "blob" || third_part == "-" {
                return format!("https://gitlab.com/{}/{}", parts[0], parts[1]);
            }
        }
        format!("https://gitlab.com/{}/{}", parts[0], parts[1])
    } else {
        url.to_string()
    }
}

fn extract_url_from_purl(purl: &str) -> Option<String> {
    // Parse purl format: pkg:type/namespace/name@version

    // Handle GitHub purls: pkg:github/owner/repo@version
    if purl.starts_with("pkg:github/") {
        let parts: Vec<&str> = purl
            .trim_start_matches("pkg:github/")
            .split('@')
            .collect();
        return Some(format!("https://github.com/{}", parts[0]));
    }

    // Handle GitLab purls
    if purl.starts_with("pkg:gitlab/") {
        let parts: Vec<&str> = purl
            .trim_start_matches("pkg:gitlab/")
            .split('@')
            .collect();
        return Some(format!("https://gitlab.com/{}", parts[0]));
    }

    // For other package types (npm, maven, etc.), we cannot reliably extract repo URL
    // Would need external registry lookups
    None
}

/// Check if component should be processed
pub(crate) fn should_process_component(component: &DtComponent) -> bool {
    matches!(component.classifier.as_str(), "LIBRARY" | "FRAMEWORK")
}

/// Convert DT component to CLOMonitor project
pub(crate) fn convert_component_to_project(
    component: &DtComponent,
    dt_project_name: &str,
) -> Result<Project> {
    let repo_url = extract_repository_url(component).ok_or_else(|| {
        format_err!(
            "No repository URL found for component {} (classifier: {})",
            component.name,
            component.classifier
        )
    })?;

    let name = format_component_name(component);
    let display_name = format_display_name(component);
    let description = component.description.clone().unwrap_or_else(|| {
        format!(
            "Component from Dependency-Track project: {}",
            dt_project_name
        )
    });

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
        logo_url: None,
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

fn format_component_name(component: &DtComponent) -> String {
    let parts = vec![
        component.group.as_deref(),
        Some(component.name.as_str()),
        component.version.as_deref(),
    ];

    parts
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
        .replace('/', "-")
        .replace('@', "-")
        .replace(':', "-")
}

fn format_display_name(component: &DtComponent) -> String {
    match (&component.group, &component.version) {
        (Some(group), Some(version)) => format!("{}/{} {}", group, component.name, version),
        (Some(group), None) => format!("{}/{}", group, component.name),
        (None, Some(version)) => format!("{} {}", component.name, version),
        (None, None) => component.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_component() -> DtComponent {
        DtComponent {
            uuid: "test-uuid".to_string(),
            name: "test-component".to_string(),
            version: Some("1.0.0".to_string()),
            group: None,
            purl: None,
            description: None,
            classifier: "LIBRARY".to_string(),
            external_references: None,
        }
    }

    // Phase 3.1 Tests - Repository URL Extraction

    #[test]
    fn test_extract_github_url_from_vcs_reference() {
        let component = DtComponent {
            external_references: Some(vec![ExternalReference {
                type_: "vcs".to_string(),
                url: "https://github.com/lodash/lodash".to_string(),
            }]),
            ..default_component()
        };

        let url = extract_repository_url(&component);
        assert_eq!(url, Some("https://github.com/lodash/lodash".to_string()));
    }

    #[test]
    fn test_extract_github_url_from_github_purl() {
        let component = DtComponent {
            purl: Some("pkg:github/microsoft/typescript@5.0.0".to_string()),
            external_references: None,
            ..default_component()
        };

        let url = extract_repository_url(&component);
        assert_eq!(
            url,
            Some("https://github.com/microsoft/typescript".to_string())
        );
    }

    #[test]
    fn test_extract_gitlab_url_from_purl() {
        let component = DtComponent {
            purl: Some("pkg:gitlab/gitlab-org/gitlab@15.0.0".to_string()),
            external_references: None,
            ..default_component()
        };

        let url = extract_repository_url(&component);
        assert_eq!(
            url,
            Some("https://gitlab.com/gitlab-org/gitlab".to_string())
        );
    }

    #[test]
    fn test_no_repository_url_returns_none() {
        let component = DtComponent {
            name: "lodash".to_string(),
            purl: Some("pkg:npm/lodash@4.17.21".to_string()), // npm purl doesn't resolve
            external_references: None,
            ..default_component()
        };

        assert_eq!(extract_repository_url(&component), None);
    }

    #[test]
    fn test_normalize_git_urls() {
        assert_eq!(
            normalize_git_url("https://github.com/user/repo.git"),
            "https://github.com/user/repo"
        );
        assert_eq!(
            normalize_git_url("https://github.com/user/repo/"),
            "https://github.com/user/repo"
        );
        assert_eq!(
            normalize_git_url("git@github.com:user/repo.git"),
            "https://github.com/user/repo"
        );
    }

    #[test]
    fn test_clean_monorepo_urls() {
        // Test cleaning submodule paths
        assert_eq!(
            normalize_git_url("https://github.com/dropwizard/metrics/metrics-core"),
            "https://github.com/dropwizard/metrics"
        );
        assert_eq!(
            normalize_git_url("https://github.com/google/gson/gson"),
            "https://github.com/google/gson"
        );
        assert_eq!(
            normalize_git_url("https://github.com/swagger-api/swagger-core/modules/swagger-core"),
            "https://github.com/swagger-api/swagger-core"
        );

        // Test cleaning tree/blob paths
        assert_eq!(
            normalize_git_url("https://github.com/apache/httpcomponents-core/tree/4.4.16/httpcore"),
            "https://github.com/apache/httpcomponents-core"
        );
        assert_eq!(
            normalize_git_url("https://github.com/yarnpkg/yarn/blob/master/packages/lockfile"),
            "https://github.com/yarnpkg/yarn"
        );

        // Test http to https conversion
        assert_eq!(
            normalize_git_url("http://github.com/FasterXML/jackson-modules-java8/jackson-module-parameter-names"),
            "https://github.com/FasterXML/jackson-modules-java8"
        );
    }

    // Phase 3.2 Tests - Component to Project Conversion

    #[test]
    fn test_should_process_library_components() {
        let library = DtComponent {
            classifier: "LIBRARY".to_string(),
            ..default_component()
        };
        assert!(should_process_component(&library));

        let framework = DtComponent {
            classifier: "FRAMEWORK".to_string(),
            ..default_component()
        };
        assert!(should_process_component(&framework));
    }

    #[test]
    fn test_should_not_process_non_library_components() {
        let container = DtComponent {
            classifier: "CONTAINER".to_string(),
            ..default_component()
        };
        assert!(!should_process_component(&container));

        let application = DtComponent {
            classifier: "APPLICATION".to_string(),
            ..default_component()
        };
        assert!(!should_process_component(&application));
    }

    #[test]
    fn test_convert_component_to_project() {
        let dt_component = DtComponent {
            uuid: "comp-123".to_string(),
            name: "lodash".to_string(),
            version: Some("4.17.21".to_string()),
            group: Some("npm".to_string()),
            description: Some("Lodash modular utilities.".to_string()),
            classifier: "LIBRARY".to_string(),
            external_references: Some(vec![ExternalReference {
                type_: "vcs".to_string(),
                url: "https://github.com/lodash/lodash".to_string(),
            }]),
            purl: Some("pkg:npm/lodash@4.17.21".to_string()),
        };

        let dt_project_name = "my-app";
        let project = convert_component_to_project(&dt_component, dt_project_name).unwrap();

        assert_eq!(project.name, "npm-lodash-4.17.21");
        assert_eq!(project.display_name, Some("npm/lodash 4.17.21".to_string()));
        assert_eq!(project.description, "Lodash modular utilities.");
        assert_eq!(project.repositories.len(), 1);
        assert_eq!(
            project.repositories[0].url,
            "https://github.com/lodash/lodash"
        );
        assert_eq!(
            project.repositories[0].check_sets,
            Some(vec!["code".to_string()])
        );
        assert!(project.digest.is_some());
    }

    #[test]
    fn test_convert_component_without_repo_url_fails() {
        let dt_component = DtComponent {
            name: "internal-lib".to_string(),
            external_references: None,
            purl: None,
            ..default_component()
        };

        let result = convert_component_to_project(&dt_component, "my-app");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No repository URL"));
    }

    #[test]
    fn test_format_component_name_variations() {
        let comp1 = DtComponent {
            name: "lodash".to_string(),
            group: Some("npm".to_string()),
            version: Some("4.17.21".to_string()),
            ..default_component()
        };
        assert_eq!(format_component_name(&comp1), "npm-lodash-4.17.21");

        let comp2 = DtComponent {
            name: "lodash".to_string(),
            group: None,
            version: Some("4.17.21".to_string()),
            ..default_component()
        };
        assert_eq!(format_component_name(&comp2), "lodash-4.17.21");

        let comp3 = DtComponent {
            name: "lodash".to_string(),
            group: None,
            version: None,
            ..default_component()
        };
        assert_eq!(format_component_name(&comp3), "lodash");
    }

    // Phase 3.3 Tests - Repository URL Lookup with Database Integration

    #[tokio::test]
    async fn test_lookup_from_db_mapping() {
        use crate::db::MockDB;
        use std::sync::Arc;

        // Setup: Component with purl that has a DB mapping
        let component = DtComponent {
            uuid: "comp-uuid-1".to_string(),
            name: "express".to_string(),
            version: Some("4.18.2".to_string()),
            group: Some("npm".to_string()),
            purl: Some("pkg:npm/express@4.18.2".to_string()),
            description: None,
            classifier: "LIBRARY".to_string(),
            external_references: None, // No VCS reference
        };

        // Mock DB to return a mapping
        let mut mock_db = MockDB::new();
        mock_db
            .expect_get_component_mapping()
            .with(mockall::predicate::eq("pkg:npm/express@4.18.2"))
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some("https://github.com/expressjs/express".to_string())) }));

        let db: crate::db::DynDB = Arc::new(mock_db);

        // Execute
        let result = extract_repository_url_with_lookup(&component, &db).await;

        // Assert: Should return Found with DB URL
        match result {
            crate::registrar::RepositoryLookupResult::Found(url) => {
                assert_eq!(url, "https://github.com/expressjs/express");
            }
            _ => panic!("Expected Found result, got NotFound"),
        }
    }

    #[tokio::test]
    async fn test_lookup_auto_discovery_saves_to_db() {
        use crate::db::MockDB;
        use std::sync::Arc;

        // Setup: Component with VCS reference but no DB mapping
        let component = DtComponent {
            uuid: "comp-uuid-2".to_string(),
            name: "lodash".to_string(),
            version: Some("4.17.21".to_string()),
            group: None,
            purl: Some("pkg:npm/lodash@4.17.21".to_string()),
            description: None,
            classifier: "LIBRARY".to_string(),
            external_references: Some(vec![ExternalReference {
                type_: "vcs".to_string(),
                url: "https://github.com/lodash/lodash".to_string(),
            }]),
        };

        // Mock DB
        let mut mock_db = MockDB::new();

        // First call: get_component_mapping returns None (no mapping exists)
        mock_db
            .expect_get_component_mapping()
            .with(mockall::predicate::eq("pkg:npm/lodash@4.17.21"))
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));

        // Second call: save_component_mapping should be called with auto-discovered URL
        mock_db
            .expect_save_component_mapping()
            .with(
                mockall::predicate::eq("pkg:npm/lodash@4.17.21"),
                mockall::predicate::eq("https://github.com/lodash/lodash"),
                mockall::predicate::eq("auto"),
                mockall::predicate::eq(Some(80)),
            )
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));

        let db: crate::db::DynDB = Arc::new(mock_db);

        // Execute
        let result = extract_repository_url_with_lookup(&component, &db).await;

        // Assert: Should return Found with auto-discovered URL and save to DB
        match result {
            crate::registrar::RepositoryLookupResult::Found(url) => {
                assert_eq!(url, "https://github.com/lodash/lodash");
            }
            _ => panic!("Expected Found result, got NotFound"),
        }
    }

    #[tokio::test]
    async fn test_lookup_returns_unmapped_when_not_found() {
        use crate::db::MockDB;
        use std::sync::Arc;

        // Setup: Component with no VCS reference and no DB mapping
        let component = DtComponent {
            uuid: "comp-uuid-3".to_string(),
            name: "internal-lib".to_string(),
            version: Some("1.0.0".to_string()),
            group: Some("com.mycompany".to_string()),
            purl: Some("pkg:maven/com.mycompany/internal-lib@1.0.0".to_string()),
            description: Some("Internal library".to_string()),
            classifier: "LIBRARY".to_string(),
            external_references: None, // No VCS reference
        };

        // Mock DB to return no mapping
        let mut mock_db = MockDB::new();
        mock_db
            .expect_get_component_mapping()
            .with(mockall::predicate::eq(
                "pkg:maven/com.mycompany/internal-lib@1.0.0",
            ))
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));

        let db: crate::db::DynDB = Arc::new(mock_db);

        // Execute
        let result = extract_repository_url_with_lookup(&component, &db).await;

        // Assert: Should return NotFound with UnmappedComponent
        match result {
            crate::registrar::RepositoryLookupResult::NotFound(unmapped) => {
                assert_eq!(unmapped.component_uuid, "comp-uuid-3");
                assert_eq!(unmapped.component_name, "internal-lib");
                assert_eq!(unmapped.component_version, Some("1.0.0".to_string()));
                assert_eq!(unmapped.component_group, Some("com.mycompany".to_string()));
                assert_eq!(
                    unmapped.purl,
                    Some("pkg:maven/com.mycompany/internal-lib@1.0.0".to_string())
                );
                assert_eq!(unmapped.classifier, "LIBRARY");
                assert_eq!(
                    unmapped.mapping_notes,
                    "No VCS reference or purl mapping found"
                );
                assert!(unmapped.external_references.is_none());
            }
            _ => panic!("Expected NotFound result, got Found"),
        }
    }

    #[tokio::test]
    async fn test_lookup_prefers_db_over_auto_discovery() {
        use crate::db::MockDB;
        use std::sync::Arc;

        // Setup: Component has BOTH DB mapping AND VCS reference
        // Should prefer DB mapping over auto-discovery
        let component = DtComponent {
            uuid: "comp-uuid-4".to_string(),
            name: "react".to_string(),
            version: Some("18.2.0".to_string()),
            group: None,
            purl: Some("pkg:npm/react@18.2.0".to_string()),
            description: None,
            classifier: "LIBRARY".to_string(),
            // VCS reference points to one URL
            external_references: Some(vec![ExternalReference {
                type_: "vcs".to_string(),
                url: "https://github.com/facebook/react".to_string(),
            }]),
        };

        // Mock DB returns a DIFFERENT URL (to verify DB takes priority)
        let mut mock_db = MockDB::new();
        mock_db
            .expect_get_component_mapping()
            .with(mockall::predicate::eq("pkg:npm/react@18.2.0"))
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    // DB has a manual override pointing to a different repo
                    Ok(Some("https://github.com/reactjs/react-monorepo".to_string()))
                })
            });

        // save_component_mapping should NOT be called since DB mapping exists
        mock_db
            .expect_save_component_mapping()
            .times(0);

        let db: crate::db::DynDB = Arc::new(mock_db);

        // Execute
        let result = extract_repository_url_with_lookup(&component, &db).await;

        // Assert: Should return DB URL, NOT the auto-discovered URL
        match result {
            crate::registrar::RepositoryLookupResult::Found(url) => {
                assert_eq!(url, "https://github.com/reactjs/react-monorepo");
                // Verify it's NOT the auto-discovered URL
                assert_ne!(url, "https://github.com/facebook/react");
            }
            _ => panic!("Expected Found result with DB URL, got NotFound"),
        }
    }

    #[tokio::test]
    async fn test_lookup_without_purl_skips_db_lookup() {
        use crate::db::MockDB;
        use std::sync::Arc;

        // Setup: Component without purl should skip DB lookup and use auto-discovery
        let component = DtComponent {
            uuid: "comp-uuid-5".to_string(),
            name: "vue".to_string(),
            version: Some("3.3.4".to_string()),
            group: None,
            purl: None, // No purl
            description: None,
            classifier: "FRAMEWORK".to_string(),
            external_references: Some(vec![ExternalReference {
                type_: "vcs".to_string(),
                url: "https://github.com/vuejs/core".to_string(),
            }]),
        };

        // Mock DB should NOT be called for get_component_mapping
        let mut mock_db = MockDB::new();
        mock_db
            .expect_get_component_mapping()
            .times(0); // Should not be called without purl

        // save_component_mapping should also NOT be called (no purl to save)
        mock_db
            .expect_save_component_mapping()
            .times(0);

        let db: crate::db::DynDB = Arc::new(mock_db);

        // Execute
        let result = extract_repository_url_with_lookup(&component, &db).await;

        // Assert: Should return Found with auto-discovered URL
        match result {
            crate::registrar::RepositoryLookupResult::Found(url) => {
                assert_eq!(url, "https://github.com/vuejs/core");
            }
            _ => panic!("Expected Found result from auto-discovery, got NotFound"),
        }
    }

    #[tokio::test]
    async fn test_lookup_db_error_falls_back_to_auto_discovery() {
        use crate::db::MockDB;
        use std::sync::Arc;
        use anyhow::format_err;

        // Setup: Component with purl and VCS reference
        let component = DtComponent {
            uuid: "comp-uuid-6".to_string(),
            name: "axios".to_string(),
            version: Some("1.4.0".to_string()),
            group: None,
            purl: Some("pkg:npm/axios@1.4.0".to_string()),
            description: None,
            classifier: "LIBRARY".to_string(),
            external_references: Some(vec![ExternalReference {
                type_: "vcs".to_string(),
                url: "https://github.com/axios/axios".to_string(),
            }]),
        };

        // Mock DB to return an error on get_component_mapping
        let mut mock_db = MockDB::new();
        mock_db
            .expect_get_component_mapping()
            .with(mockall::predicate::eq("pkg:npm/axios@1.4.0"))
            .times(1)
            .returning(|_| Box::pin(async { Err(format_err!("Database connection error")) }));

        // save_component_mapping should still be called with auto-discovered URL
        mock_db
            .expect_save_component_mapping()
            .with(
                mockall::predicate::eq("pkg:npm/axios@1.4.0"),
                mockall::predicate::eq("https://github.com/axios/axios"),
                mockall::predicate::eq("auto"),
                mockall::predicate::eq(Some(80)),
            )
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));

        let db: crate::db::DynDB = Arc::new(mock_db);

        // Execute
        let result = extract_repository_url_with_lookup(&component, &db).await;

        // Assert: Should fall back to auto-discovery when DB errors
        match result {
            crate::registrar::RepositoryLookupResult::Found(url) => {
                assert_eq!(url, "https://github.com/axios/axios");
            }
            _ => panic!("Expected Found result from auto-discovery fallback, got NotFound"),
        }
    }

    #[tokio::test]
    async fn test_lookup_saves_github_purl_mapping() {
        use crate::db::MockDB;
        use std::sync::Arc;

        // Setup: Component with GitHub purl (no VCS reference)
        let component = DtComponent {
            uuid: "comp-uuid-7".to_string(),
            name: "typescript".to_string(),
            version: Some("5.0.0".to_string()),
            group: None,
            purl: Some("pkg:github/microsoft/typescript@5.0.0".to_string()),
            description: None,
            classifier: "LIBRARY".to_string(),
            external_references: None, // No VCS reference
        };

        // Mock DB
        let mut mock_db = MockDB::new();

        // get_component_mapping returns None
        mock_db
            .expect_get_component_mapping()
            .with(mockall::predicate::eq("pkg:github/microsoft/typescript@5.0.0"))
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));

        // save_component_mapping should be called with URL extracted from purl
        mock_db
            .expect_save_component_mapping()
            .with(
                mockall::predicate::eq("pkg:github/microsoft/typescript@5.0.0"),
                mockall::predicate::eq("https://github.com/microsoft/typescript"),
                mockall::predicate::eq("auto"),
                mockall::predicate::eq(Some(80)),
            )
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));

        let db: crate::db::DynDB = Arc::new(mock_db);

        // Execute
        let result = extract_repository_url_with_lookup(&component, &db).await;

        // Assert: Should auto-discover from purl and save to DB
        match result {
            crate::registrar::RepositoryLookupResult::Found(url) => {
                assert_eq!(url, "https://github.com/microsoft/typescript");
            }
            _ => panic!("Expected Found result from purl auto-discovery, got NotFound"),
        }
    }
}
