use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

/// HTTP client for communicating with clomonitor-importer API
pub(crate) struct ImporterClient {
    base_url: String,
    http_client: reqwest::Client,
}

impl ImporterClient {
    /// Creates a new ImporterClient with the given base URL
    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Builds a URL for the given API path
    fn build_url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Checks if the importer API is healthy
    pub async fn health_check(&self) -> Result<bool> {
        let url = self.build_url("/api/health/status");
        debug!("Health check: {}", url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to send health check request")?;

        Ok(response.status().is_success())
    }

    /// Gets the total count of projects
    pub async fn get_project_count(&self) -> Result<u32> {
        let url = self.build_url("/api/export/projects/count");
        debug!("Getting project count from: {}", url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to send project count request")?;

        let count_response: ProjectCountResponse = response
            .json()
            .await
            .context("Failed to parse project count response")?;

        debug!("Total project count: {}", count_response.total);
        Ok(count_response.total)
    }

    /// Gets a page of projects with optional foundation filter
    pub async fn get_projects(
        &self,
        foundation: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<ExportProjectsResponse> {
        let mut url = format!(
            "{}/api/export/projects?offset={}&limit={}",
            self.base_url, offset, limit
        );

        if let Some(foundation_name) = foundation {
            url.push_str(&format!("&foundation={}", foundation_name));
        }

        debug!("Getting projects from: {}", url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to send get projects request")?;

        let projects_response: ExportProjectsResponse = response
            .json()
            .await
            .context("Failed to parse projects response")?;

        debug!(
            "Retrieved {} projects (offset={}, limit={})",
            projects_response.projects.len(),
            offset,
            limit
        );

        Ok(projects_response)
    }

    /// Gets all projects by automatically handling pagination
    pub async fn get_all_projects(
        &self,
        foundation: Option<&str>,
        page_size: u32,
    ) -> Result<Vec<ExportProject>> {
        debug!(
            "Getting all projects (foundation={:?}, page_size={})",
            foundation, page_size
        );

        let mut all_projects = Vec::new();
        let mut offset = 0;

        loop {
            let response = self
                .get_projects(foundation, offset, page_size)
                .await
                .with_context(|| {
                    format!(
                        "Failed to fetch projects page at offset {} for foundation {:?}",
                        offset, foundation
                    )
                })?;

            let num_projects = response.projects.len();
            all_projects.extend(response.projects);

            // If we got fewer projects than the page size, we've reached the end
            if num_projects < page_size as usize {
                break;
            }

            offset += page_size;
        }

        debug!("Retrieved total of {} projects", all_projects.len());
        Ok(all_projects)
    }
}

// Response types based on API schema
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ExportProjectsResponse {
    pub foundation: Option<String>,
    pub total: u32,
    pub offset: u32,
    pub limit: u32,
    pub projects: Vec<ExportProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ExportProject {
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub maturity: Option<String>,
    pub logo_url: Option<String>,
    pub devstats_url: Option<String>,
    pub repositories: Vec<ExportRepository>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ExportRepository {
    pub name: String,
    pub url: String,
    #[serde(rename = "checkSets")]
    pub check_sets: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ProjectCountResponse {
    pub total: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_importer_client_creation() {
        let base_url = "https://importer.example.com".to_string();
        let client = ImporterClient::new(base_url.clone());

        // Verify client is created with correct base_url
        assert_eq!(client.base_url, base_url);
    }

    #[tokio::test]
    async fn test_health_check_returns_true_when_api_healthy() {
        // Setup mock server
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/health/status")
            .with_status(200)
            .create_async()
            .await;

        let client = ImporterClient::new(server.url());
        let result = client.health_check().await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_health_check_returns_false_when_api_unhealthy() {
        // Setup mock server
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/health/status")
            .with_status(503)
            .create_async()
            .await;

        let client = ImporterClient::new(server.url());
        let result = client.health_check().await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_project_count_returns_total() {
        // Setup mock server
        let mut server = mockito::Server::new_async().await;
        let response_body = r#"{"total": 42}"#;
        let mock = server
            .mock("GET", "/api/export/projects/count")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response_body)
            .create_async()
            .await;

        let client = ImporterClient::new(server.url());
        let result = client.get_project_count().await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_projects_without_foundation() {
        // Setup mock server
        let mut server = mockito::Server::new_async().await;
        let response_body = r#"{
            "foundation": null,
            "total": 2,
            "offset": 0,
            "limit": 10,
            "projects": [
                {
                    "name": "project1",
                    "display_name": "Project One",
                    "description": "First project",
                    "category": "App Definition",
                    "maturity": "graduated",
                    "logo_url": "https://example.com/logo1.png",
                    "devstats_url": "https://devstats.example.com/project1",
                    "repositories": [
                        {
                            "name": "repo1",
                            "url": "https://github.com/org/repo1",
                            "checkSets": ["code", "community"]
                        }
                    ]
                },
                {
                    "name": "project2",
                    "display_name": null,
                    "description": null,
                    "category": null,
                    "maturity": null,
                    "logo_url": null,
                    "devstats_url": null,
                    "repositories": []
                }
            ]
        }"#;

        let mock = server
            .mock("GET", "/api/export/projects?offset=0&limit=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response_body)
            .create_async()
            .await;

        let client = ImporterClient::new(server.url());
        let result = client.get_projects(None, 0, 10).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.total, 2);
        assert_eq!(response.offset, 0);
        assert_eq!(response.limit, 10);
        assert_eq!(response.foundation, None);
        assert_eq!(response.projects.len(), 2);
        assert_eq!(response.projects[0].name, "project1");
        assert_eq!(response.projects[0].repositories.len(), 1);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_projects_with_foundation() {
        // Setup mock server
        let mut server = mockito::Server::new_async().await;
        let response_body = r#"{
            "foundation": "cncf",
            "total": 1,
            "offset": 5,
            "limit": 20,
            "projects": [
                {
                    "name": "kubernetes",
                    "display_name": "Kubernetes",
                    "description": "Container orchestration",
                    "category": "Orchestration",
                    "maturity": "graduated",
                    "logo_url": "https://example.com/k8s.png",
                    "devstats_url": "https://k8s.devstats.cncf.io",
                    "repositories": [
                        {
                            "name": "kubernetes",
                            "url": "https://github.com/kubernetes/kubernetes",
                            "checkSets": ["code", "community", "docs"]
                        }
                    ]
                }
            ]
        }"#;

        let mock = server
            .mock(
                "GET",
                "/api/export/projects?offset=5&limit=20&foundation=cncf",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response_body)
            .create_async()
            .await;

        let client = ImporterClient::new(server.url());
        let result = client.get_projects(Some("cncf"), 5, 20).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.total, 1);
        assert_eq!(response.offset, 5);
        assert_eq!(response.limit, 20);
        assert_eq!(response.foundation, Some("cncf".to_string()));
        assert_eq!(response.projects.len(), 1);
        assert_eq!(response.projects[0].name, "kubernetes");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_all_projects_handles_pagination() {
        // Setup mock server with multiple pages
        let mut server = mockito::Server::new_async().await;

        // First page
        let page1_body = r#"{
            "foundation": null,
            "total": 5,
            "offset": 0,
            "limit": 2,
            "projects": [
                {
                    "name": "project1",
                    "display_name": "Project 1",
                    "description": null,
                    "category": null,
                    "maturity": null,
                    "logo_url": null,
                    "devstats_url": null,
                    "repositories": []
                },
                {
                    "name": "project2",
                    "display_name": "Project 2",
                    "description": null,
                    "category": null,
                    "maturity": null,
                    "logo_url": null,
                    "devstats_url": null,
                    "repositories": []
                }
            ]
        }"#;

        // Second page
        let page2_body = r#"{
            "foundation": null,
            "total": 5,
            "offset": 2,
            "limit": 2,
            "projects": [
                {
                    "name": "project3",
                    "display_name": "Project 3",
                    "description": null,
                    "category": null,
                    "maturity": null,
                    "logo_url": null,
                    "devstats_url": null,
                    "repositories": []
                },
                {
                    "name": "project4",
                    "display_name": "Project 4",
                    "description": null,
                    "category": null,
                    "maturity": null,
                    "logo_url": null,
                    "devstats_url": null,
                    "repositories": []
                }
            ]
        }"#;

        // Third page
        let page3_body = r#"{
            "foundation": null,
            "total": 5,
            "offset": 4,
            "limit": 2,
            "projects": [
                {
                    "name": "project5",
                    "display_name": "Project 5",
                    "description": null,
                    "category": null,
                    "maturity": null,
                    "logo_url": null,
                    "devstats_url": null,
                    "repositories": []
                }
            ]
        }"#;

        let mock1 = server
            .mock("GET", "/api/export/projects?offset=0&limit=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(page1_body)
            .create_async()
            .await;

        let mock2 = server
            .mock("GET", "/api/export/projects?offset=2&limit=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(page2_body)
            .create_async()
            .await;

        let mock3 = server
            .mock("GET", "/api/export/projects?offset=4&limit=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(page3_body)
            .create_async()
            .await;

        let client = ImporterClient::new(server.url());
        let result = client.get_all_projects(None, 2).await;

        assert!(result.is_ok());
        let all_projects = result.unwrap();
        assert_eq!(all_projects.len(), 5);
        assert_eq!(all_projects[0].name, "project1");
        assert_eq!(all_projects[1].name, "project2");
        assert_eq!(all_projects[2].name, "project3");
        assert_eq!(all_projects[3].name, "project4");
        assert_eq!(all_projects[4].name, "project5");

        mock1.assert_async().await;
        mock2.assert_async().await;
        mock3.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_all_projects_with_foundation() {
        // Setup mock server with single page for foundation
        let mut server = mockito::Server::new_async().await;
        let response_body = r#"{
            "foundation": "cncf",
            "total": 2,
            "offset": 0,
            "limit": 10,
            "projects": [
                {
                    "name": "kubernetes",
                    "display_name": "Kubernetes",
                    "description": null,
                    "category": null,
                    "maturity": null,
                    "logo_url": null,
                    "devstats_url": null,
                    "repositories": []
                },
                {
                    "name": "prometheus",
                    "display_name": "Prometheus",
                    "description": null,
                    "category": null,
                    "maturity": null,
                    "logo_url": null,
                    "devstats_url": null,
                    "repositories": []
                }
            ]
        }"#;

        let mock = server
            .mock(
                "GET",
                "/api/export/projects?offset=0&limit=10&foundation=cncf",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response_body)
            .create_async()
            .await;

        let client = ImporterClient::new(server.url());
        let result = client.get_all_projects(Some("cncf"), 10).await;

        assert!(result.is_ok());
        let all_projects = result.unwrap();
        assert_eq!(all_projects.len(), 2);
        assert_eq!(all_projects[0].name, "kubernetes");
        assert_eq!(all_projects[1].name, "prometheus");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_projects_handles_network_error() {
        // Setup mock server but don't create any mocks - request will fail
        let server = mockito::Server::new_async().await;

        let client = ImporterClient::new(server.url());
        let result = client.get_projects(None, 0, 10).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_projects_handles_invalid_json() {
        // Setup mock server with invalid JSON response
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/export/projects?offset=0&limit=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("invalid json {]")
            .create_async()
            .await;

        let client = ImporterClient::new(server.url());
        let result = client.get_projects(None, 0, 10).await;

        assert!(result.is_err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_project_count_handles_server_error() {
        // Setup mock server with 500 error
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/export/projects/count")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let client = ImporterClient::new(server.url());
        let result = client.get_project_count().await;

        // Should return error since response parsing will fail
        assert!(result.is_err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_health_check_handles_connection_error() {
        // Use invalid URL that will fail to connect
        let client =
            ImporterClient::new("http://invalid-url-that-does-not-exist.local".to_string());
        let result = client.health_check().await;

        assert!(result.is_err());
    }
}
