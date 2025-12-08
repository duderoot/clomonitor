use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, warn};

use super::{RegistryApi, normalize_git_url, parse_npm_purl};

const NPM_REGISTRY_URL: &str = "https://registry.npmjs.org";

#[derive(Deserialize)]
struct NpmPackageInfo {
    repository: Option<RepositoryField>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RepositoryField {
    Object { url: String },
    String(String),
}

pub struct NpmRegistry {
    client: Client,
    base_url: String,
}

impl NpmRegistry {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: NPM_REGISTRY_URL.to_string(),
        }
    }

    #[cfg(test)]
    pub fn new_with_base_url(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    async fn fetch_package_info(&self, package_name: &str) -> Result<Option<NpmPackageInfo>> {
        let url = format!("{}/{}", self.base_url, package_name);
        debug!("Fetching npm package info from: {}", url);

        let response = self.client.get(&url).send().await?;

        if response.status().as_u16() == 404 {
            debug!("Package not found: {}", package_name);
            return Ok(None);
        }

        let package_info: NpmPackageInfo = response.json().await?;
        Ok(Some(package_info))
    }

    fn extract_repository_url(&self, repository: &RepositoryField) -> Option<String> {
        let url_str = match repository {
            RepositoryField::Object { url } => url.as_str(),
            RepositoryField::String(s) => s.as_str(),
        };

        normalize_git_url(url_str)
    }
}

#[async_trait]
impl RegistryApi for NpmRegistry {
    async fn lookup_repository(&self, purl: &str) -> Result<Option<String>> {
        let (package_name, _version) = parse_npm_purl(purl)?;
        debug!("Looking up npm package: {}", package_name);

        let Some(package_info) = self.fetch_package_info(&package_name).await? else {
            return Ok(None);
        };

        if let Some(repository) = package_info.repository {
            if let Some(url) = self.extract_repository_url(&repository) {
                debug!("Found repository URL: {}", url);
                return Ok(Some(url));
            } else {
                warn!(
                    "Repository field present but couldn't extract valid git URL for {}",
                    package_name
                );
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito;

    #[tokio::test]
    async fn test_npm_lookup_with_github_repo() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/lodash")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(include_str!("testdata/npm_lodash.json"))
            .create_async()
            .await;

        let client = NpmRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:npm/lodash@4.17.21")
            .await
            .unwrap();

        assert_eq!(result, Some("https://github.com/lodash/lodash".to_string()));
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_npm_lookup_with_gitlab_repo() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/gitlab-package")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(include_str!("testdata/npm_gitlab.json"))
            .create_async()
            .await;

        let client = NpmRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:npm/gitlab-package@2.0.0")
            .await
            .unwrap();

        assert_eq!(
            result,
            Some("https://gitlab.com/gitlab-org/gitlab".to_string())
        );
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_npm_lookup_no_repository_field() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/no-repo-package")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(include_str!("testdata/npm_no_repo.json"))
            .create_async()
            .await;

        let client = NpmRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:npm/no-repo-package@1.0.0")
            .await
            .unwrap();

        assert_eq!(result, None);
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_npm_lookup_404() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/nonexistent")
            .with_status(404)
            .create_async()
            .await;

        let client = NpmRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:npm/nonexistent@1.0.0")
            .await
            .unwrap();

        assert_eq!(result, None);
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_npm_handles_string_repository() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/string-repo")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(include_str!("testdata/npm_string_repo.json"))
            .create_async()
            .await;

        let client = NpmRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:npm/string-repo@1.0.0")
            .await
            .unwrap();

        assert_eq!(result, Some("https://github.com/user/repo".to_string()));
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_npm_scoped_package() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/@types/node")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"repository": {"type": "git", "url": "https://github.com/DefinitelyTyped/DefinitelyTyped.git"}}"#,
            )
            .create_async()
            .await;

        let client = NpmRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:npm/%40types/node@18.0.0")
            .await
            .unwrap();

        assert_eq!(
            result,
            Some("https://github.com/DefinitelyTyped/DefinitelyTyped".to_string())
        );
        mock_server.assert_async().await;
    }
}
