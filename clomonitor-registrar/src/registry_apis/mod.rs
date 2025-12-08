use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait RegistryApi: Send + Sync {
    /// Look up repository URL for a package identified by PURL
    async fn lookup_repository(&self, purl: &str) -> Result<Option<String>>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegistryType {
    Npm,
    Maven,
    PyPI,
}

pub struct RegistryRouter {
    npm: Option<Arc<dyn RegistryApi>>,
    maven: Option<Arc<dyn RegistryApi>>,
    pypi: Option<Arc<dyn RegistryApi>>,
}

impl RegistryRouter {
    pub fn new(
        npm: Option<Arc<dyn RegistryApi>>,
        maven: Option<Arc<dyn RegistryApi>>,
        pypi: Option<Arc<dyn RegistryApi>>,
    ) -> Self {
        Self { npm, maven, pypi }
    }

    pub async fn lookup(&self, purl: &str) -> Result<Option<String>> {
        match detect_registry(purl) {
            Some(RegistryType::Npm) => {
                if let Some(ref npm) = self.npm {
                    npm.lookup_repository(purl).await
                } else {
                    Ok(None)
                }
            }
            Some(RegistryType::Maven) => {
                if let Some(ref maven) = self.maven {
                    maven.lookup_repository(purl).await
                } else {
                    Ok(None)
                }
            }
            Some(RegistryType::PyPI) => {
                if let Some(ref pypi) = self.pypi {
                    pypi.lookup_repository(purl).await
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }
}

/// Detect which registry a PURL belongs to
pub(crate) fn detect_registry(purl: &str) -> Option<RegistryType> {
    if purl.starts_with("pkg:npm/") {
        Some(RegistryType::Npm)
    } else if purl.starts_with("pkg:maven/") {
        Some(RegistryType::Maven)
    } else if purl.starts_with("pkg:pypi/") {
        Some(RegistryType::PyPI)
    } else {
        None
    }
}

/// Parse npm PURL into (package_name, version)
/// Handles scoped packages with URL encoding: pkg:npm/%40types/node@18.0.0
pub(crate) fn parse_npm_purl(purl: &str) -> Result<(String, String)> {
    let without_prefix = purl
        .strip_prefix("pkg:npm/")
        .ok_or_else(|| anyhow!("Not a valid npm PURL"))?;

    let parts: Vec<&str> = without_prefix.split('@').collect();

    match parts.len() {
        2 => {
            // Simple package: lodash@4.17.21
            let name = urlencoding::decode(parts[0])?.into_owned();
            let version = parts[1].to_string();
            Ok((name, version))
        }
        3 => {
            // Scoped package: @types/node@18.0.0
            // parts = ["", "types/node", "18.0.0"]
            let name = format!("@{}", parts[1]);
            let name = urlencoding::decode(&name)?.into_owned();
            let version = parts[2].to_string();
            Ok((name, version))
        }
        _ => Err(anyhow!("Invalid npm PURL format")),
    }
}

/// Parse Maven PURL into (group_id, artifact_id, version)
/// Strips qualifiers: pkg:maven/org.springframework/spring-core@5.3.0?type=jar
pub(crate) fn parse_maven_purl(purl: &str) -> Result<(String, String, String)> {
    let without_prefix = purl
        .strip_prefix("pkg:maven/")
        .ok_or_else(|| anyhow!("Not a valid Maven PURL"))?;

    // Strip qualifiers (everything after ?)
    let without_qualifiers = without_prefix.split('?').next().unwrap_or(without_prefix);

    let parts: Vec<&str> = without_qualifiers.split('@').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid Maven PURL format"));
    }

    let ga_parts: Vec<&str> = parts[0].split('/').collect();
    if ga_parts.len() != 2 {
        return Err(anyhow!("Invalid Maven group/artifact format"));
    }

    let group_id = ga_parts[0].to_string();
    let artifact_id = ga_parts[1].to_string();
    let version = parts[1].to_string();

    Ok((group_id, artifact_id, version))
}

/// Parse PyPI PURL into (package_name, version)
pub(crate) fn parse_pypi_purl(purl: &str) -> Result<(String, String)> {
    let without_prefix = purl
        .strip_prefix("pkg:pypi/")
        .ok_or_else(|| anyhow!("Not a valid PyPI PURL"))?;

    let parts: Vec<&str> = without_prefix.split('@').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid PyPI PURL format"));
    }

    let name = parts[0].to_string();
    let version = parts[1].to_string();

    Ok((name, version))
}

/// Normalize various git URL formats to standard https:// URLs
/// Returns None if the URL is not recognized as a git repository
pub(crate) fn normalize_git_url(url: &str) -> Option<String> {
    let url = url.trim();

    // Handle github: shorthand
    if let Some(path) = url.strip_prefix("github:") {
        return Some(format!("https://github.com/{}", path));
    }

    // Handle gitlab: shorthand
    if let Some(path) = url.strip_prefix("gitlab:") {
        return Some(format!("https://gitlab.com/{}", path));
    }

    // Convert git:// to https://
    let url = if url.starts_with("git://") {
        url.replace("git://", "https://")
    } else if url.starts_with("git+https://") {
        url.replace("git+https://", "https://")
    } else if url.starts_with("git+ssh://git@") {
        url.replace("git+ssh://git@", "https://")
    } else {
        url.to_string()
    };

    // Strip .git suffix
    let url = if url.ends_with(".git") {
        url.strip_suffix(".git").unwrap().to_string()
    } else {
        url
    };

    // Validate it's a known git hosting service
    if url.contains("github.com/") || url.contains("gitlab.com/") || url.contains("bitbucket.org/")
    {
        Some(url)
    } else {
        None
    }
}

pub mod maven;
pub mod npm;
pub mod pypi;

pub use npm::NpmRegistry;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_npm_purl() {
        // Simple package
        let (name, version) = parse_npm_purl("pkg:npm/lodash@4.17.21").unwrap();
        assert_eq!(name, "lodash");
        assert_eq!(version, "4.17.21");

        // Scoped package with URL encoding
        let (name, version) = parse_npm_purl("pkg:npm/%40types/node@18.0.0").unwrap();
        assert_eq!(name, "@types/node");
        assert_eq!(version, "18.0.0");
    }

    #[test]
    fn test_parse_maven_purl() {
        // Basic Maven PURL
        let (group, artifact, version) =
            parse_maven_purl("pkg:maven/org.springframework/spring-core@5.3.0").unwrap();
        assert_eq!(group, "org.springframework");
        assert_eq!(artifact, "spring-core");
        assert_eq!(version, "5.3.0");

        // With qualifiers (should strip them)
        let (group, artifact, version) =
            parse_maven_purl("pkg:maven/org.springframework/spring-core@5.3.0?type=jar").unwrap();
        assert_eq!(group, "org.springframework");
        assert_eq!(artifact, "spring-core");
        assert_eq!(version, "5.3.0");
    }

    #[test]
    fn test_parse_pypi_purl() {
        let (name, version) = parse_pypi_purl("pkg:pypi/requests@2.28.0").unwrap();
        assert_eq!(name, "requests");
        assert_eq!(version, "2.28.0");
    }

    #[test]
    fn test_normalize_git_url() {
        // git:// protocol
        assert_eq!(
            normalize_git_url("git://github.com/lodash/lodash.git"),
            Some("https://github.com/lodash/lodash".to_string())
        );

        // git+https:// protocol
        assert_eq!(
            normalize_git_url("git+https://github.com/lodash/lodash.git"),
            Some("https://github.com/lodash/lodash".to_string())
        );

        // Standard https with .git
        assert_eq!(
            normalize_git_url("https://github.com/lodash/lodash.git"),
            Some("https://github.com/lodash/lodash".to_string())
        );

        // Already normalized
        assert_eq!(
            normalize_git_url("https://github.com/lodash/lodash"),
            Some("https://github.com/lodash/lodash".to_string())
        );

        // GitHub shorthand
        assert_eq!(
            normalize_git_url("github:lodash/lodash"),
            Some("https://github.com/lodash/lodash".to_string())
        );

        // GitLab
        assert_eq!(
            normalize_git_url("https://gitlab.com/user/repo.git"),
            Some("https://gitlab.com/user/repo".to_string())
        );

        // Non-git URLs should return None
        assert_eq!(normalize_git_url("https://example.com"), None);
    }

    #[test]
    fn test_detect_registry_from_purl() {
        assert_eq!(
            detect_registry("pkg:npm/lodash@4.17.21"),
            Some(RegistryType::Npm)
        );
        assert_eq!(
            detect_registry("pkg:maven/org.foo/bar@1.0"),
            Some(RegistryType::Maven)
        );
        assert_eq!(
            detect_registry("pkg:pypi/requests@2.0"),
            Some(RegistryType::PyPI)
        );
        assert_eq!(detect_registry("pkg:docker/alpine@latest"), None);
    }
}
