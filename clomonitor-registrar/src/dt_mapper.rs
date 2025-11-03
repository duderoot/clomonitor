use anyhow::{format_err, Result};
use crate::dt_types::DtComponent;
use crate::registrar::{Project, Repository};

#[cfg(test)]
use crate::dt_types::ExternalReference;

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

    // Remove .git suffix
    if normalized.ends_with(".git") {
        normalized.truncate(normalized.len() - 4);
    }

    // Convert git@ URLs to https
    if normalized.starts_with("git@github.com:") {
        normalized = normalized
            .replace("git@github.com:", "https://github.com/");
    }

    normalized
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
}
