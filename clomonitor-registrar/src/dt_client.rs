use anyhow::Result;
use async_trait::async_trait;
use reqwest::StatusCode;
use tracing::debug;

use crate::dt_types::{DtComponent, DtProject};

const PAGE_SIZE: u32 = 100;

#[async_trait]
pub(crate) trait DtClient {
    async fn get_projects(&self) -> Result<Vec<DtProject>>;
    async fn get_project_components(&self, project_uuid: &str) -> Result<Vec<DtComponent>>;
}

pub(crate) struct DtHttpClient {
    base_url: String,
    api_key: String,
    http_client: reqwest::Client,
}

impl DtHttpClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http_client: reqwest::Client::new(),
        }
    }

    fn build_url(&self, path: &str) -> String {
        format!("{}/api{}", self.base_url, path)
    }
}

#[async_trait]
impl DtClient for DtHttpClient {
    async fn get_projects(&self) -> Result<Vec<DtProject>> {
        let mut all_projects = Vec::new();
        let mut page = 1;

        loop {
            let url = self.build_url(&format!(
                "/v1/project?pageNumber={}&pageSize={}",
                page, PAGE_SIZE
            ));

            debug!("Fetching projects page {} from {}", page, url);

            let response = self
                .http_client
                .get(&url)
                .header("X-Api-Key", &self.api_key)
                .send()
                .await?;

            if response.status() != StatusCode::OK {
                return Err(anyhow::format_err!(
                    "DT API error: status={}, body={}",
                    response.status(),
                    response.text().await?
                ));
            }

            let total_count = response
                .headers()
                .get("X-Total-Count")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);

            let projects: Vec<DtProject> = response.json().await?;
            let fetched = projects.len();
            all_projects.extend(projects);

            debug!(
                "Fetched {} projects (total so far: {}/{})",
                fetched,
                all_projects.len(),
                total_count
            );

            // Stop if we've fetched everything
            if all_projects.len() >= total_count {
                break;
            }

            // Stop if we got no results on this page
            if fetched == 0 {
                break;
            }

            page += 1;
        }

        Ok(all_projects)
    }

    async fn get_project_components(&self, project_uuid: &str) -> Result<Vec<DtComponent>> {
        let mut all_components = Vec::new();
        let mut page = 1;

        loop {
            let url = self.build_url(&format!(
                "/v1/component/project/{}?pageNumber={}&pageSize={}",
                project_uuid, page, PAGE_SIZE
            ));

            debug!(
                "Fetching components page {} for project {}",
                page, project_uuid
            );

            let response = self
                .http_client
                .get(&url)
                .header("X-Api-Key", &self.api_key)
                .send()
                .await?;

            if response.status() != StatusCode::OK {
                return Err(anyhow::format_err!(
                    "DT API error: status={}, body={}",
                    response.status(),
                    response.text().await?
                ));
            }

            let total_count = response
                .headers()
                .get("X-Total-Count")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);

            let components: Vec<DtComponent> = response.json().await?;
            let fetched = components.len();
            all_components.extend(components);

            debug!(
                "Fetched {} components (total so far: {}/{})",
                fetched,
                all_components.len(),
                total_count
            );

            // Stop if we've fetched everything
            if all_components.len() >= total_count {
                break;
            }

            // Stop if we got no results on this page
            if fetched == 0 {
                break;
            }

            page += 1;
        }

        Ok(all_components)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito;

    #[tokio::test]
    async fn test_get_projects_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/project")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body(r#"[{"uuid":"123","name":"test-project","version":"1.0"}]"#)
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "test-key".to_string());
        let projects = client.get_projects().await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "test-project");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_projects_pagination() {
        let mut server = mockito::Server::new_async().await;

        // First page
        let mock1 = server
            .mock("GET", "/api/v1/project")
            .match_query(mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()))
            .with_status(200)
            .with_header("X-Total-Count", "2")
            .with_body(r#"[{"uuid":"1","name":"proj1"}]"#)
            .create_async()
            .await;

        // Second page
        let mock2 = server
            .mock("GET", "/api/v1/project")
            .match_query(mockito::Matcher::UrlEncoded("pageNumber".into(), "2".into()))
            .with_status(200)
            .with_header("X-Total-Count", "2")
            .with_body(r#"[{"uuid":"2","name":"proj2"}]"#)
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "test-key".to_string());
        let projects = client.get_projects().await.unwrap();

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "proj1");
        assert_eq!(projects[1].name, "proj2");
        mock1.assert_async().await;
        mock2.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_projects_api_error() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/api/v1/project")
            .with_status(403)
            .with_body("Forbidden")
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "bad-key".to_string());
        let result = client.get_projects().await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // Check that error contains status code or error info
        assert!(err_msg.contains("API error") || err_msg.contains("status="), "Error message was: {}", err_msg);
    }

    #[tokio::test]
    async fn test_get_project_components() {
        let mut server = mockito::Server::new_async().await;
        let project_uuid = "proj-123";

        let mock = server
            .mock("GET", format!("/api/v1/component/project/{}", project_uuid).as_str())
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body(
                r#"[{"uuid":"comp-1","name":"lodash","version":"4.17.21","classifier":"LIBRARY"}]"#,
            )
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "test-key".to_string());
        let components = client.get_project_components(project_uuid).await.unwrap();

        assert_eq!(components.len(), 1);
        assert_eq!(components[0].name, "lodash");
        mock.assert_async().await;
    }
}
