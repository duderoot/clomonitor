use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, warn};

use super::{RegistryApi, normalize_git_url, parse_pypi_purl};

const PYPI_API_URL: &str = "https://pypi.org/pypi";

#[derive(Deserialize)]
struct PyPIPackageInfo {
    info: PyPIInfo,
}

#[derive(Deserialize)]
struct PyPIInfo {
    #[serde(default)]
    project_urls: Option<HashMap<String, String>>,
    #[serde(default)]
    home_page: Option<String>,
}

pub struct PyPIRegistry {
    client: Client,
    base_url: String,
}

impl PyPIRegistry {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: PYPI_API_URL.to_string(),
        }
    }

    #[cfg(test)]
    pub fn new_with_base_url(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    async fn fetch_package_info(&self, package_name: &str) -> Result<Option<PyPIPackageInfo>> {
        let url = format!("{}/{}/json", self.base_url, package_name);
        debug!("Fetching PyPI package info from: {}", url);

        let response = self.client.get(&url).send().await?;

        if response.status().as_u16() == 404 {
            debug!("Package not found: {}", package_name);
            return Ok(None);
        }

        let package_info: PyPIPackageInfo = response.json().await?;
        Ok(Some(package_info))
    }

    fn extract_repository_url(&self, info: &PyPIInfo) -> Option<String> {
        // Priority 1: Check project_urls for known repository keys
        if let Some(ref project_urls) = info.project_urls {
            // Try common repository keys
            for key in &["Source", "Repository", "Code", "source", "repository"] {
                if let Some(url) = project_urls.get(*key) {
                    if let Some(normalized) = normalize_git_url(url) {
                        return Some(normalized);
                    }
                }
            }

            // Check Homepage in project_urls as fallback
            if let Some(url) = project_urls.get("Homepage") {
                if let Some(normalized) = normalize_git_url(url) {
                    return Some(normalized);
                }
            }
        }

        // Priority 2: Check home_page field
        if let Some(ref home_page) = info.home_page {
            if let Some(normalized) = normalize_git_url(home_page) {
                return Some(normalized);
            }
        }

        None
    }
}

#[async_trait]
impl RegistryApi for PyPIRegistry {
    async fn lookup_repository(&self, purl: &str) -> Result<Option<String>> {
        let (package_name, _version) = parse_pypi_purl(purl)?;
        debug!("Looking up PyPI package: {}", package_name);

        let Some(package_info) = self.fetch_package_info(&package_name).await? else {
            return Ok(None);
        };

        if let Some(url) = self.extract_repository_url(&package_info.info) {
            debug!("Found repository URL: {}", url);
            return Ok(Some(url));
        } else {
            warn!(
                "No valid repository URL found for PyPI package: {}",
                package_name
            );
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito;

    #[tokio::test]
    async fn test_pypi_lookup_with_project_urls() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/requests/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(include_str!("testdata/pypi_requests.json"))
            .create_async()
            .await;

        let client = PyPIRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:pypi/requests@2.28.0")
            .await
            .unwrap();

        assert_eq!(result, Some("https://github.com/psf/requests".to_string()));
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_pypi_lookup_with_gitlab() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/gitlab-package/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(include_str!("testdata/pypi_gitlab.json"))
            .create_async()
            .await;

        let client = PyPIRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:pypi/gitlab-package@3.0.0")
            .await
            .unwrap();

        assert_eq!(
            result,
            Some("https://gitlab.com/gitlab-org/python-gitlab".to_string())
        );
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_pypi_lookup_with_homepage_only() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/homepage-only/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(include_str!("testdata/pypi_homepage_only.json"))
            .create_async()
            .await;

        let client = PyPIRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:pypi/homepage-only@1.5.0")
            .await
            .unwrap();

        assert_eq!(
            result,
            Some("https://github.com/user/homepage-only".to_string())
        );
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_pypi_lookup_no_repository() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/no-repo-package/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(include_str!("testdata/pypi_no_repo.json"))
            .create_async()
            .await;

        let client = PyPIRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:pypi/no-repo-package@1.0.0")
            .await
            .unwrap();

        assert_eq!(result, None);
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_pypi_lookup_404() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/nonexistent/json")
            .with_status(404)
            .create_async()
            .await;

        let client = PyPIRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:pypi/nonexistent@1.0.0")
            .await
            .unwrap();

        assert_eq!(result, None);
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_pypi_filters_non_git_urls() {
        let json = r#"{
            "info": {
                "name": "docs-only",
                "version": "1.0.0",
                "home_page": "https://readthedocs.io/projects/myproject"
            }
        }"#;

        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/docs-only/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json)
            .create_async()
            .await;

        let client = PyPIRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:pypi/docs-only@1.0.0")
            .await
            .unwrap();

        // Should return None since readthedocs.io is not a git hosting service
        assert_eq!(result, None);
        mock_server.assert_async().await;
    }
}
